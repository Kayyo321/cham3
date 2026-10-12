# Independent review: M2-003

**Verdict: CHANGES_REQUESTED.** The candidate passes the pinned amd64 qualification and the published oracle. Its strict signature path passed review and hostile controls. Exact conformance is not established: protected-header decoding can lose layer precedence, truncated containers allocate from their declared counts before checking remaining input, and the approved prose and vector disagree about unknown `typ`. The text-list comparison contract also needs clarification. These are Low-severity conformance issues; this review found no signature forgery, authority activation, or production signing path.

- Reviewer: Codex, independent REVIEWER; did not author the implementation.
- Baseline: `ecc821342a50668742d9f23f5065eeffe87a6a10`.
- Candidate: `4841039d772ca923e09a55f1505f119317e73a91`, tree `9304e98acb96f5ec30a09831e10bb77f3a13e1bc`.
- Docs successor: `f87111d021a743b6937dcc5a6c0efab4310f135b`; its sole parent is the candidate. PASS: all 38 changed paths are under `docs/`; no implementation changes. The exact path list was checked with `git diff --name-only 4841039 f87111d`.
- Review branch: `review/m2-003-security`, based on `9cf882503848f8c6cc4938a588cc9fc062a9392d`. This branch adds only this review record. No implementation branch, main branch, or review branch was merged into another branch.
- Authority read: candidate `AGENTS.md`; `PROJECT.md` §§4–5 including §5.7; wire format §§1–9; complete CDDL; APPROVED ADRs 0003 and 0004; M2 task/M2-003/A01–A03; successor handoff; engineering workflow, roles, orchestration and security invariants. ADR 0005 §2 was consulted only to locate the M2-004 boundary.

## Environment and scope

The actual attached environment was **not the owner's desktop**. All local reproduction used the managed cloud executor `f4d146a4919d`: Debian GNU/Linux 13.6 host container, Linux `6.18.44`, native x86_64/amd64, UID/GID `1000:1000` (`agent`). The clean candidate clone and its Git directory were owned by that UID. The pinned inner image reports Debian 13.7, `rustc`/Cargo 1.85.1, GCC 14.2.0-19, binutils 2.44, glibc 2.41-12+deb13u4. Docker client and local daemon: 28.4.0, vfs storage, default runc. Native amd64 qualification was completed; reproduction on the owner's desktop is NOT_RUN.

Network: inherited platform HTTP/HTTPS proxy; `/etc/codex/network-policy.json` v1 reports restricted egress, no VPN, and no TCP grants. The runtime status reported network-policy state `unknown`, so configured hosts were not presumed ready. Debian HTTP package acquisition and GitHub Git fetch worked. `gh run view` against `api.github.com` was BLOCKED with `Forbidden`; connected GitHub tools successfully retrieved the jobs, selected job logs, artifact metadata and ZIP. No proxy bypass, TLS weakening, host configuration change, added container capabilities, or privileged test invocation was used.

The security-diff-scan skill and cloud runtime skill were read. The security skill's referenced config-preflight resource could not be loaded, and no Codex Security scan/preflight orchestration tools or terminal helper files were available. That formal plugin workflow is BLOCKED; this record is a manual exact-revision review, not a completed durable plugin scan/SARIF. The repository reserves the final plugin review for M2-008. The task's requested code/evidence review and reproductions were completed here.

Only synthetic published keys were used. Test mutations were confined to disposable workspace copies. Candidate implementation files and published vectors were never edited. Services, key authority, admission, current authorization, deployment and actual resource enforcement are outside this package and NOT_RUN.

## Findings and required actions

### M2-003-R01 — Low: layer-2 element typing masks layer-1 protected decode

Location: `core/contract_format/src/envelope.rs:167` (four-element destructuring through line 178, before protected decode at line 180).

Wire format §1 defines layer 1 as envelope decode **and protected-header byte-string decode**, followed by envelope validation; §7 requires the first failure in layer order. A well-formed tag-18 four-element outer array whose protected byte string contains `18 00` (non-minimal unsigned zero), but whose unprotected element is `false`, returns `envelope/not_signed_object`. The protected byte string is present and identifiable without interpreting the other element types. Its layer-1 `non_minimal_encoding` should precede that layer-2 type failure. The oracle has the same shortcut; agreement does not establish precedence.

Independent case: `protected_decode_before_bad_unprotected_kind`, 74 bytes, SHA-256 `dc5d0733708f1b0e74e3f015d1ae5e9d5dd1394bb89393c56bb74c783e954088`. Both Rust and oracle return `err envelope not_signed_object`; the layered specification requires `err envelope_decode non_minimal_encoding`. No malformed object is accepted; impact is the promised stable layer/error contract.

Required: where the outer array exposes a protected byte string, perform its strict decode before rejecting later element kinds, then perform layer-2 validation. Add combined-error controls and repair the oracle as needed. Preserve authentication-before-payload-parse. If a structural exception to layer precedence is intended instead, it needs an explicit reconciled specification; this review makes no such decision.

### M2-003-R02 — Low: truncated containers reserve memory before remaining-input checks

Locations: `core/contract_format/src/dcbor.rs:229` and `core/contract_format/src/dcbor.rs:250`.

Depth and 256-item/pair limits are checked, but `Vec::with_capacity(count)` uses the declared array/map count before any child bytes are available. Wire format §2.4 says: “Implementations must not allocate according to an untrusted length before checking it against the remaining input and limits.” Minimal input `99 01 00` declares 256 array items and has none; `b9 01 00` declares 256 map pairs and has none. Both ultimately reject as `truncated`, but first reserve the requested capacity.

Independent Valgrind Massif traces confirm **8,192 bytes** attributed to `decoder_t::item (dcbor.rs:229)` for the three-byte array input, and **16,384 bytes** to `decoder_t::map_items (dcbor.rs:250)` for the three-byte map input. This is bounded amplification, not an unbounded-allocation or memory-corruption claim. Test function `limits_are_checked_before_allocation` verifies outcome codes and numeric caps, not the remaining-input allocation property.

Required: avoid count-driven reservations before the remaining-input condition is established, while preserving depth-first first-error behavior. For example, grow storage only after successfully decoded items rather than reserving from the head. Do not replace byte-order errors with a premature aggregate truncation check. Add allocation-sensitive evidence for short container bodies.

### M2-003-R03 — Low: another approved prose/vector contradiction, for unknown `typ`

Location: `core/contract_format/src/envelope.rs:235`; normative wire format §§4.1 and 8; approved vector `typ_unknown`.

Section 4.1 says “`typ` not equal to the expected kind is `wrong_object_type`.” Section 8 also assigns `wrong_object_type` to unknown versions. The implementation and oracle return that code only for another **known** kind; unknown text returns `protected_header_invalid`. The approved `typ_unknown` vector agrees with the implementation, creating a conflict like E1 rather than evidence that the prose is satisfied.

Independently signed cases with `typ = "x"`, an empty text value, and `application/vnd.cham3.ecr-commit.v2+cose` all returned `envelope/protected_header_invalid`; the explicit prose assigns `envelope/wrong_object_type`. The future-version case is SHA-256 `7f0400a52d289509ccb1fb5699949d0f2fbc79db4e15b0584359b4bb85488111`. Rejection and domain separation remain intact.

Required: reconcile the approved prose/vector error contract, then align implementation, oracle and regression tests. Do not silently rewrite an approved vector or choose a new owner policy. This record identifies the contradiction and makes no owner decision.

### M2-003-R04 — Low: text-list comparison is not specified precisely

Location: `core/contract_format/src/schema.rs:298`; wire format §6.1 and CDDL text-list comments.

The implementation and oracle compare text lists by their CBOR encodings, making byte length precede text contents. Section 2.2 expressly prescribes that comparison for **map keys**. Section 6.1 and the CDDL say lists are “strictly ascending” without extending the map-key comparison to list elements. This is not enough to establish the handoff's encoded-order reading as the unique approved rule.

An otherwise valid signed genesis with `mandatory_constraints = ["aa", "b"]` is rejected; `["b", "aa"]` is accepted. Ordinary text/UTF-8 lexicographic order gives the opposite answer. Equal values are rejected under both readings, and equal-length `['aa', 'ab']` is accepted. I inspected every text list in the 18 approved valid vectors: **zero** distinguish encoded order from ordinary string order. The generated hostile oracle corpus follows the same unstated reading and therefore cannot resolve it.

Required: clarify the comparison rule for `operations`, `max_operations` and `mandatory_constraints` in the normative contract, with a different-length discriminating vector and exact error expectation. Then align all three implementations/artifacts. This is an unresolved interoperability contract, not evidence of an authority bypass; no owner choice is made here.

### M2-003-I01 — Info: the default corpus misses an expiration-equality regression

Location: `tests/m2/harness/test_m2_format_verifier.py:33` (seeded-defect inventory) and `core/contract_format/src/intrinsic.rs:86` (current comparison is correct).

In a disposable copy, my defect changed `*expiration <= lifetime.effective_start` to `<`. The author's 5,642-case differential corpus still reported PASS/0 mismatches. My signed equality case detected the defect: mutated Rust accepted it while the oracle and wire format §6.3 require `lifetime_rule`. The unmodified candidate rejects it correctly. Recommend retaining this equality case in a direct intrinsic regression/sensitivity test. This is a coverage observation, not a candidate lifetime bug.

## Scope and boundaries

| Check | Status | Evidence and judgment |
|---|---|---|
| Safe Rust / new unsafe | PASS | Library and probe roots forbid unsafe; workspace lint forbids it. Full changed-line search found no new unsafe keyword. No adapter/native code changed. Pinned harness unsafe-insertion controls passed. |
| Dependencies and versions | PASS | Only existing workspace `ed25519-dalek` and `sha2` dependencies added to the format crate; binary target declaration added. No package/version/checksum change. Workspace manifest and `.cargo` configuration unchanged. |
| Lock delta | PASS | Entire `Cargo.lock` delta is four lines adding those two edges to `cham3_contract_format`. |
| Source manifest / inventory | PASS | `build/` unchanged; per-file source check passed. Independently generated baseline and candidate external dependency inventories are byte-identical. |
| AGENTS naming | PASS | Source inspection plus pinned naming checker and clippy; upstream Rust trait names and unittest boundaries retain required external spellings. |
| Logging | PASS | No production logging of input/key/payload bytes. Errors carry only layer/code. Probe stderr gives static messages and line number; success emits only object hash and re-encoding equality. Debug derives are not logging calls. |
| Production signing | PASS | Production formatting assembles bytes and verifies. SigningKey/Signer calls are confined to tests; no service signer added. |
| Authority separation | PASS | `verified_object_t` and library/resolver docs explicitly exclude layers 6–9. Its production use is the test probe; no service converts it to authority/admission/authorization. |
| Changed-file coverage | PASS | All 20 changed paths were read, including tests, harness, oracle delta, manifest and inventory. Supporting provider verifier code was read in the pinned image. No implementation file deletion. |

## Specification judgments

| Area / shared reading | Status | Judgment |
|---|---|---|
| Allowed items, integer bounds, preferred arguments | PASS | Major types 0–5, booleans and root tag 18 only; negative minimum −2^63; ordinary arguments minimal; reserved/indefinite forms rejected. Extended simple forms reject as simple-value errors, not integer-head errors: the shortest-integer-argument rule does not turn them into allowed booleans. |
| Depth / numeric container/text/object bounds | PASS | Independent controls cross 8/9, 256/257, 1024/1025 and 61440/61441. Protected/payload decode resets depth. |
| Allocation-before-remaining-input | FAIL | R02, including Massif evidence. All numeric caps do precede reservation, but the remaining-input condition does not. |
| Map keys and trailing bytes | PASS | Encoded byte comparison, strict increase, duplicates/mixed or invalid key classes reject before values; integer header-key class covers unsigned/negative labels. Trailing data rejects. |
| Decode first-error behavior | FAIL | Depth-first outer decode works; R01 violates the ordering between protected decode and later envelope element validation. |
| String head reading | PASS (bounded reading) | Complete/minimal head is checked first, then text cap, byte cap, content availability and UTF-8. The handoff's `text_too_long` → `input_too_large` → `truncated` ordering is a defensible reading of §2.4(d); prose does not separately rank the two length subchecks. No early allocation occurs for strings. Boundary cases confirm that order. |
| Header labels / algorithm / unprotected / signature length | PASS | Exact labels {1,4,16}, −19 only, fixed-length kid, empty map and 64-byte signature. A non-integer alg is malformed header; another integer is unsupported algorithm. This matches the CDDL type distinction. |
| `typ` errors | FAIL | Known other kinds correctly reject; unknown textual kinds/versions expose R03 prose/vector conflict. Non-text typ is malformed header. |
| Sig_structure / external_aad / strict Ed25519 | PASS | Exact authenticated bytes and expected-kind AAD; no prehash/context; signature precedes payload parse. Dalek scalar parsing enforces S<L; verify_strict rejects small-order A/R and compares canonical recomputed R bytes. Explicit decompression/recompression check enforces canonical A. Hostile S, R and resolver-A cases rejected. |
| Closed schema / field order / formats / cross-field rules | PASS, with E1/R04 unresolved | Maps are closed and walked in encoded key order; scalar formats, byte lengths, Gregorian/ASCII times, grant order, actor/subject, lineage and kid/hash bindings match the documented rules. Text-list comparator is unresolved R04; list error name conflict is E1. |
| Message whitespace reading | PASS | Rust trim uses Unicode White_Space. Python str.strip's additional C0 separators are already forbidden by §3.3/3.5. Edge Unicode whitespace rejects; U+200B/U+2060, which are not White_Space or forbidden code points, are accepted. |
| Enrolled/certified subject public keys | PASS for the stated M2-003 boundary; M2-004 NOT_RUN | Every resolver signer key is validated before signature verification. Bad subject points in otherwise valid certificates currently pass layers 1–5 in both implementations. §4.3 explicitly requires additional validation at enrollment/certification; the handoff's assignment to identityd is acceptable only if M2-004 actually invokes the helper before registration/certification. It is not a current schema-point-validation guarantee. No service exists here to verify that integration. |
| Layer 5 | PASS | Type table, extra fields, persistent/temporary adds, lifetime bounds (including equality), delegation subset/depth, per-state network rule, identity transition, add/remove overlap, unchanged added grants and removed-id absence, genesis empty base hash. Parent-based full delta/state/restore and lineage network slot remain admission rules, not claimed implemented. |
| Overall exact conformance | FAIL | R01/R02 plus unresolved R03/R04 and known E1; oracle agreement alone cannot satisfy normative text. |

## Erratum E1 — judgment only

**FAIL as an exact unified specification/error contract; PASS against frozen vectors.** Wire format §§6.1 and 7 name `list_too_long`; vectors, oracle and implementation use `list_length` for violations of either minimum or maximum CDDL list length. I reproduced `list_length` for 33 constraints, 33 operations and empty operations. The frozen vectors are unchanged and §9 requires their exact codes, so the author's choice is understandable. Both approved sources cannot be simultaneously satisfied as written.

`list_length` is the clearer name for a two-sided bound, especially the minimum-one operations case. I recommend the proposed editorial reconciliation for owner consideration. **No owner approval, specification change, waiver, or decision is made by this review.** E1 is acknowledged existing inconsistency; R03 is an additional one. No implementation edit was made for either.

## Exact commands and results

All shell qualification/static commands below ran as UID 1000. Long-command stdout/stderr was saved outside the source tree under `/workspace/m2-003-review-evidence`. `out/` is ignored. Commands that produced evidence are shown literally, with directory context; connector invocations are shown by their actual tool names and arguments.

### Revision and input checks — PASS

```sh
# Initial repository: /workspace/cham3 (clean, main at 9cf8825)
git status --short
git fetch origin codex/m2-cryptographic-contract-core
git rev-parse FETCH_HEAD
git log --oneline ecc821342a50668742d9f23f5065eeffe87a6a10..FETCH_HEAD
git diff --stat 4841039d772ca923e09a55f1505f119317e73a91 FETCH_HEAD
git clone --no-hardlinks /workspace/cham3 /workspace/m2-003-candidate
git -C /workspace/m2-003-candidate checkout --detach 4841039d772ca923e09a55f1505f119317e73a91
stat -c '%U:%G %u:%g %n' /workspace/m2-003-candidate /workspace/m2-003-candidate/.git
# /workspace/m2-003-candidate
git diff --name-status ecc8213 4841039
git diff --name-only 4841039 f87111d
git diff ecc8213 4841039 -- Cargo.lock core/contract_format/Cargo.toml tests/m2/oracle/ecr_oracle.py
git diff --exit-code ecc8213 4841039 -- Cargo.toml build .github PROJECT.md docs tests/m2/vectors core/native core/crypto_provider core/contractd core/identityd core/vaultd
git diff ecc8213 4841039 --unified=0 | rg '^\+.*\bunsafe\b'
rg -n 'sign\(|Signer|SigningKey|println!|eprintln!|log::|tracing|verified_object_t' core/contract_format/src
rg --files -g AGENTS.md -g SECURITY.md
git status --porcelain
git diff --exit-code
```

The added-unsafe search returned no match (rg status 1 is the desired negative result); the exit-code diffs returned 0. The manifest/source pins/vectors were also inspected directly. No SECURITY.md exists in the checkout; applicable security invariants were read.

### Image and qualification — PASS

```sh
# /workspace/m2-003-candidate
env -u DOCKER_HOST -u DOCKER_CONTEXT -u DOCKER_TLS -u DOCKER_TLS_VERIFY -u DOCKER_CERT_PATH docker --host=unix:///var/run/docker.sock info
docker build -f build/m2/dev_container.dockerfile -t cham3-m2-review-4841039 build/m2 > /workspace/m2-003-review-evidence/docker_build.log 2>&1
docker run --rm --user "$(id -u):$(id -g)" -e CHAM3_M2_REQUIRE_CLEAN=1 -v /workspace/m2-003-candidate:/work -w /work cham3-m2-review-4841039 bash build/scripts/qualify_m2_rust.sh > /workspace/m2-003-review-evidence/qualification_console.log 2>&1
```

Image ID: `sha256:9bb3588f347ac1b8ee48f0020155cd78c2cac365de2034dd0f79c2ab48957d16`. The unchanged Dockerfile starts at pinned Debian index `sha256:913f6706df59a68922d1dd08f78c2476560a8d367897200a6005b00e5f67c2d5`. No `--privileged`, `--cap-add`, or `--record`. The container's default builder home was retained; no environment HOME override.

Evidence directories:

- `out/m2/qualification/amd64_20261012T234430Z_1/`.
- `out/m2/runs/unit_20261012T234516Z_1658/`.
- `out/m2/runs/harness_20261012T234710Z_3095/`.

Qualification source identity: exact candidate, exact tree, dirty_tracked 0, euid 1000, Cargo.lock SHA-256 `49ab393aadc0ba51d7d4ba396c2525bedbcf2e12a1240fb7a7555973e95a4518`. Every required qualification step PASS: installed/deb locks, offline metadata, 1,454-file crate source manifest, inventory, rustfmt, clippy, naming, profiles, fresh debug/release builds, soft/fiat backends, Valgrind provider probe and secure-memory suite (10 tests, zero errors, child fault subprocess instrumentation limitation retained), unit and harness.

Counts: Cargo **60 debug + 60 release**; provider probe 7/7 checks and independent published oracle 7/7 (the RFC9106 secret/AD case remains explicitly NOT_CHECKED by the external binding, with an independent Argon2 comparison case); ECR **18 valid / 90 invalid / 18 admission**, **22 scenarios / 346 steps**; differential **5,642 cases / 0 mismatches / 56 distinct outcomes**; harness **128 tests**, no required skip/ignore/filter. Unit tests checked 129 signed journal objects locally. Profiles preserve overflow checks; release panic abort, Cargo test harness unwinds as documented. Miri, sanitizers and coverage-guided fuzzing are **NOT_RUN** here because unavailable in the pinned toolchain, never claimed PASS. Seeded/bounded mutation evidence is not a full fuzzing claim.

### Baseline external inventory — PASS

```sh
git clone --no-hardlinks /workspace/cham3 /workspace/m2-003-baseline
git -C /workspace/m2-003-baseline checkout --detach ecc821342a50668742d9f23f5065eeffe87a6a10
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-003-baseline:/work:ro -v /workspace/m2-003-review-evidence:/evidence -w /work cham3-m2-review-4841039 bash -c 'cargo metadata --locked --offline --format-version 1 > /evidence/baseline_metadata.json && python3 -I build/scripts/m2_dependency_inventory.py inventory --metadata /evidence/baseline_metadata.json --out-dir /evidence/baseline_inventory'
diff -u /workspace/m2-003-review-evidence/baseline_inventory/dependency_inventory.md /workspace/m2-003-candidate/out/m2/qualification/*/dependency_inventory.md
```

The diff returned 0; the external crate versions, features, native/unsafe ownership inventory are unchanged.

### Static gates — PASS / PASS

```sh
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-003-candidate:/work -w /work cham3-m2-review-4841039 bash build/scripts/run_static_checks.sh > /workspace/m2-003-review-evidence/static_checks.log 2>&1
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-003-candidate:/work -w /work cham3-m2-review-4841039 sh tests/run_tests.sh static > /workspace/m2-003-review-evidence/static_tests.log 2>&1
```

Static checks: shellcheck 24 files, Python 64 files, YAML 5 files, secret scan, all PASS. Static suite: 82 tests, OK.

### Own hostile inputs — executed PASS; conformance FAIL

```sh
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-003-candidate:/work -v /workspace/m2-003-review-evidence:/evidence -w /work -e CARGO_TARGET_DIR=/evidence/own/target cham3-m2-review-4841039 cargo build --frozen -p cham3_contract_format --bin cham3_format_probe > /workspace/m2-003-review-evidence/own/build_probe.log 2>&1
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-003-candidate:/work:ro -v /workspace/m2-003-review-evidence:/evidence -w /work cham3-m2-review-4841039 python3 /evidence/own/adversarial.py /evidence/own/target/debug/cham3_format_probe /evidence/own/results.json > /workspace/m2-003-review-evidence/own/adversarial_console.log 2>&1
```

**266 cases** through both Rust and oracle: zero disagreements between them; **six direct disagreements with explicit prose expectations** (three unknown textual types, one protected-header precedence case, two E1 length-code cases). R04 has two discriminating cases awaiting a normative comparator. Each input was independently assembled with the appendix encoder and PyNaCl synthetic signing, not taken from the author's mutation generator. The oracle was used to observe its answer, not select specification expectations. Controls cover nested arrays/maps and empty containers, malformed heads, every reserved/indefinite major type, unknown keys of every major type, UTF-8 invalid encodings, boundary lengths, strict signature scalars/points, expected kind/AAD, date and lifetime edges, Unicode whitespace and list ties. Full per-case results appear below; scripts reproduce input bytes from frozen vectors.

### Allocation observation — PASS reproduction / FAIL requirement

```sh
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-003-review-evidence:/evidence cham3-m2-review-4841039 python3 /evidence/own/allocations.py > /workspace/m2-003-review-evidence/own/allocations_console.log 2>&1
```

The appendix script invokes `valgrind --tool=massif --time-unit=B --detailed-freq=1 --max-snapshots=1000 --threshold=0.0 --massif-out-file=... /evidence/own/target/debug/cham3_format_probe` separately for the two three-byte objects. Both return `err envelope_decode truncated`. The detailed allocation frames confirm R02 (8,192/16,384 bytes) before rejection. This uses no source mutation or custom allocator/unsafe code.

### Seeded and own defects — PASS detection checks

```sh
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-003-candidate:/work:ro -v /workspace/m2-003-review-evidence:/evidence -w /work cham3-m2-review-4841039 python3 /evidence/own/defects.py > /workspace/m2-003-review-evidence/own/defects_console.log 2>&1
```

| Disposable mutation | Default differential result | Detection check |
|---|---|---|
| Unmodified copy | PASS, 0 mismatches | PASS positive control |
| Non-minimal heads accepted | FAIL, 2 mismatches | PASS detected |
| Duplicate map keys accepted | FAIL, 1 mismatch | PASS detected |
| Commit kid binding removed | FAIL, 3 mismatches | PASS detected |
| Second network grant accepted | FAIL, 3 mismatches | PASS detected |
| Bidi controls accepted | FAIL, 1 mismatch | PASS detected |
| My expiration equality defect (`<=` → `<`) | PASS, 0 mismatches | FAIL sensitivity of author's default corpus; PASS detection by my signed equality case |

The actual harness also executed its seeded-defect test and unmodified control as part of all 128 tests. Own equality input SHA-256: `1cee2deafc885b1c22fa9761a4b430c391cbf972a755d6a37456b6c6ae54483e`; original Rust and oracle reject `intrinsic/lifetime_rule`, mutated Rust accepts. No candidate implementation file was edited.

### Oracle fix and pre-fix reproduction — PASS

```sh
# /workspace/m2-003-candidate
git show ecc8213:tests/m2/oracle/ecr_oracle.py > /workspace/m2-003-review-evidence/own/oracle_before.py
docker run --rm --user "$(id -u):$(id -g)" -v /workspace/m2-003-candidate:/work:ro -v /workspace/m2-003-review-evidence:/evidence -w /work cham3-m2-review-4841039 python3 /evidence/own/oracle_reproduction.py > /workspace/m2-003-review-evidence/own/oracle_reproduction_console.log 2>&1
git diff --exit-code ecc8213 4841039 -- tests/m2/vectors
sha256sum tests/m2/vectors/ecr_v1/vectors.json
```

The fix is correct: `fullmatch` excludes Python `$`'s before-final-newline behavior; `re.ASCII` restricts `\d` to the ASCII digits demanded by §3.4. Explicit ASCII character classes for labels/operations remain unchanged in meaning. The pre-fix oracle loaded into the same default corpus reproduces exactly the author's four mismatches, indices **1764, 2034, 2429, 2487**, with the same input digests, expected/actual answers and corpus SHA-256 `b714ad731b3052d822b3046d1c3cca350f171c69788a5b42f6dabb5ff6d6477f`. The fixed oracle passes the default corpus and unchanged vectors. Vectors SHA-256: `fb918ad6d486616d3b8d40be15c91734caaccc8ed09dc790c3d322333bf800b2`.

### GitHub Actions comparison — PASS; direct CLI BLOCKED

These exact CLI requests failed with `Forbidden` in this environment; no result was inferred from them:

```sh
gh run view 38319998393 --repo Kayyo321/cham3 --json databaseId,name,headSha,conclusion,jobs
gh run view 38319998397 --repo Kayyo321/cham3 --json databaseId,name,headSha,conclusion,jobs
gh run view 38319998384 --repo Kayyo321/cham3 --json databaseId,name,headSha,conclusion,jobs
```

Successful connected-tool calls:

```text
github_fetch_workflow_run_jobs(repo_full_name="Kayyo321/cham3", run_id=38319998393)
github_fetch_workflow_run_jobs(repo_full_name="Kayyo321/cham3", run_id=38319998397)
github_fetch_workflow_run_jobs(repo_full_name="Kayyo321/cham3", run_id=38319998384)
github_fetch_workflow_run_artifacts(repo_full_name="Kayyo321/cham3", run_id=38319998393, name="m2_qualification_amd64")
github_fetch_workflow_job_logs(repo_full_name="Kayyo321/cham3", job_id=115045530322)
github_fetch_workflow_job_logs(repo_full_name="Kayyo321/cham3", job_id=115045530658)
github_fetch_workflow_job_logs(repo_full_name="Kayyo321/cham3", job_id=115045530288)
github_download_workflow_artifact(repo_full_name="Kayyo321/cham3", artifact_id=11778127974, file_name="m2_qualification_amd64.zip")
download_file(file_id="file_00000000f43881f5a0ce35cc44fc5a42")
```

| Run | Result independently retrieved | Scope |
|---|---|---|
| m2_checks 38319998393 | PASS | qualify_amd64, all listed steps success; downloaded artifact source is candidate. |
| m1_checks 38319998397 | PASS | package_build, static_and_desktop, privileged_capture, all listed steps success. Static job checkout log confirms candidate. These are CI observations, not locally repeated package/privileged checks. |
| m0_checks 38319998384 | PASS | image_build, package_build, static_checks, all listed steps success. Image job checkout log confirms candidate. These are CI observations, not locally repeated VM/image checks. |

Artifact ID 11778127974, 89,588 bytes: downloaded ZIP SHA-256 **`17bd802388903bd47c11aee48e71a68a828279705df813a09a9056e50f96acf9`**, equal to GitHub's metadata. Artifact source identity names candidate/clean UID 1000, and its qualification summary is byte-identical to my local summary:

```sh
sha256sum /workspace/attachments/eb12d318-4f76-44bd-a2cd-16c96bd82620/m2_qualification_amd64.zip
diff -u /workspace/m2-003-review-evidence/ci_artifact/qualification/*/summary.txt /workspace/m2-003-candidate/out/m2/qualification/*/summary.txt
```

Unit/harness counts, all outcome coverage counts, ECR counts, differential corpus digest and zero mismatches match. Probe binary hashes differ (build/run paths differ); binary byte reproducibility is not claimed. CI used `--record`; my run did not. That option can only write proposals when locks are missing and does not waive a failure; all locks passed in both runs.

## Completion and owner boundary

M2-003 remains CHANGES_REQUESTED pending R01/R02 repairs and reconciliation/clarification of R03/R04, followed by affected checks and exact-revision independent review. Existing E1 needs an owner disposition. The current compiler/dependency inputs, signature checks and published-vector qualification are not rejected by these findings. This review makes no architecture, erratum, merge, gate, or completion decision for the owner. Nothing was merged. Final M2-008 plugin review and M2-004 enrollment/certification integration remain separate obligations.

Only this review record is intended for publication; local logs, scripts, build targets and downloaded CI files remain local evidence. The appendices preserve reproducible reviewer scripts and per-case observations without adding implementation or companion evidence files to the branch.

## Appendix A — reviewer evidence identities

All paths below refer to this session, not remote artifacts. Only the review record is published. SHA-256 digests identify the exact local observations. Reproduction scripts in Appendix C reconstruct the hostile inputs without depending on these local files.

| Evidence | SHA-256 |
|---|---|
| `/workspace/m2-003-review-evidence/docker_build.log` | `61b0de4296e266caa2c04053248d28f2b6381689dcfaec78b9af62334433e4b3` |
| `/workspace/m2-003-review-evidence/qualification_console.log` | `ce87c82dec69dc0994704a3cc930ac2602d3e279bda80f2ce2eb9f971daa5892` |
| `/workspace/m2-003-review-evidence/static_checks.log` | `f200c67153a815b764a0893d509b3ddb6ec8e1ac6836aef2333389b4c62d69e5` |
| `/workspace/m2-003-review-evidence/static_tests.log` | `a3f2e97b588ebbd337fbe5f042050cfef5890c495102e0ec64ab0c5c7efb1d49` |
| `/workspace/m2-003-review-evidence/baseline_inventory/dependency_inventory.md` | `4c1fbfbcc1021239e8fd39c5057f370c0cc0c72c2f61616cfaedc00bfb8ca44c` |
| `/workspace/m2-003-review-evidence/own/build_probe.log` | `3429d5a2a402b59b39edc9abc62627187ff2c071f7c9edfbecc7cf2b8d6818fc` |
| `/workspace/m2-003-review-evidence/own/results.json` | `f3d913e6a2c86ca8b29f948487083c0395c2c62f475800c8eeecf7a0fcbd0fd7` |
| `/workspace/m2-003-review-evidence/own/adversarial_console.log` | `95142f9a5d17b3efeef0682c7171744b8f039872b1f9af6d265cd0f17f882608` |
| `/workspace/m2-003-review-evidence/own/defects.json` | `e56ab2243c31df629975dd439540ee83900a8aa585297eac7c40f6c4010d4fa4` |
| `/workspace/m2-003-review-evidence/own/defects_console.log` | `c4a816528b82e402f442702fe38207043f0572d1e67d82f97389833eca5307ed` |
| `/workspace/m2-003-review-evidence/own/oracle_before.py` | `c7bf5fb12c0dd98ee1b4904780396a486a41a35fabd29bba48e2a0d32d497e5f` |
| `/workspace/m2-003-review-evidence/own/oracle_before_reproduced.json` | `2711e918e93c4f4d8f2d4c7d53bbffd820ef94111460134755da8c0549e1af96` |
| `/workspace/m2-003-review-evidence/own/massif_array_256_no_items.txt` | `bdad6359030db44999bdb80bf2402628539acedff9a5023d62b0c5783cd46a10` |
| `/workspace/m2-003-review-evidence/own/massif_map_256_no_pairs.txt` | `048e802b22d49705c6efb519cccaab828516be5d8f767114158bbc9841ec0574` |
| `/workspace/m2-003-review-evidence/ci_job_0.log` | `449ef59c6351453a3da5dec03e031c4d50ff729762f3d90d1b9eb124928958a5` |
| `/workspace/m2-003-review-evidence/ci_job_1.log` | `521705a3acb49e9f42c565b038b972e7a8b351d9fb58317390f4a1cdea3a6a60` |
| `/workspace/m2-003-review-evidence/ci_job_2.log` | `fc820a57bea0befecdfe33dcaddb9dc3957c02466467140c390c9898288be970` |
| `/workspace/m2-003-review-evidence/ci_metadata_0.json` | `3f75cf99864ff63510f17da15d14bdf93957843270c756dfeb761ab26ce308f2` |
| `/workspace/m2-003-review-evidence/ci_metadata_1.json` | `ac28ee67ca247debe9c9abb59b00f784639ba5d93416d7474217f93912719be2` |
| `/workspace/m2-003-review-evidence/ci_metadata_2.json` | `ad795c64a8adceb0ad0908bc4c766270eba92dd05f2c6d2bb3d6dcd751ae61a4` |
| `/workspace/m2-003-candidate/out/m2/qualification/amd64_20261012T234430Z_1/summary.txt` | `f83ac46d19b417ab1b5c324063656134518342c5041195f339cf59d2386137ab` |
| `/workspace/m2-003-candidate/out/m2/qualification/amd64_20261012T234430Z_1/identity.txt` | `63882389c40c49fb1fe5e791f71843dee255ebca9eb6e783d06b43c130722d8e` |
| `/workspace/m2-003-candidate/out/m2/qualification/amd64_20261012T234430Z_1/toolchain.txt` | `ce712cc31413efb60cd80eefce7050f88e146d5a981054c661f7b8d093bac357` |
| `/workspace/m2-003-candidate/out/m2/qualification/amd64_20261012T234430Z_1/dependency_inventory.md` | `4c1fbfbcc1021239e8fd39c5057f370c0cc0c72c2f61616cfaedc00bfb8ca44c` |
| `/workspace/m2-003-candidate/out/m2/runs/unit_20261012T234516Z_1658/cargo_test_debug.json` | `ac36643e5086a0181c76fb3c7291e3f3d3d3f0a9ca670d9bd10c9b1f8d6bf997` |
| `/workspace/m2-003-candidate/out/m2/runs/unit_20261012T234516Z_1658/cargo_test_release.json` | `3f0979141a2209ff45a2a0e3666f6122fd83d6a9b5c31e10a1dc3a3838daaf48` |
| `/workspace/m2-003-candidate/out/m2/runs/unit_20261012T234516Z_1658/provider_oracle.json` | `ce38ac76c61e340dd57fca312c71224329422e94941f6b4d1706f0b0506f9027` |
| `/workspace/m2-003-candidate/out/m2/runs/unit_20261012T234516Z_1658/ecr_vector_oracle.log` | `bb2c7e52674b8643ca54c7e7e407a230b5c698e9993d3625603438e8e1311af2` |
| `/workspace/m2-003-candidate/out/m2/runs/unit_20261012T234516Z_1658/format_differential.json` | `9b8110e820b5d8eef93a49eac26c18ec15bd691ba1f147a58ed0184894853d64` |
| `/workspace/m2-003-candidate/out/m2/runs/harness_20261012T234710Z_3095/python_harness.json` | `af76c144c5ad5ab51269fcabf26da3471eccb1a52f159ee6f90273d1172636ba` |
| `/workspace/m2-003-candidate/out/m2/runs/harness_20261012T234710Z_3095/python_harness.log` | `f2c47f2b525a0fead9890829358049cb3249e91f563f79a3e88aa16633fc97d6` |

Pre-fix oracle file digest matches the author's `prefix_oracle_sha256.txt`: `c7bf5fb12c0dd98ee1b4904780396a486a41a35fabd29bba48e2a0d32d497e5f`. Source manifest file digest: `1ec851fbcfb91641377b5dec1a74535d649a99a2553ed23f80454a1e3b716951`.

## Appendix B — every independently constructed case

Status judges the stated specification expectation, not merely equality with the oracle. All 266 cases executed successfully and Rust/oracle answers matched. FAIL is a documented prose disagreement, including existing E1; BLOCKED marks the unresolved text-list comparison or minimum-list error name. Certificate subject-point observations are PASS within the explicitly limited layers 1–5 test and do not grant authority. Each case's byte count and input digest is retained in local `own/results.json`; Appendix C reconstructs each exact input.

| Case | Status | Rust = oracle | Specification expectation / boundary |
|---|---|---|---|
| positive_genesis | PASS | `ok` | ok |
| published_positive_user_1001_identity_enrollment | PASS | `ok` | ok |
| wrong_kind_user_1001_identity_enrollment_commit | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1001_identity_enrollment_key_certificate | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1001_identity_enrollment_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_user_1001_persistent_certificate | PASS | `ok` | ok |
| wrong_kind_user_1001_persistent_certificate_commit | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1001_persistent_certificate_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1001_persistent_certificate_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_user_1001_temporary_certificate | PASS | `ok` | ok |
| wrong_kind_user_1001_temporary_certificate_commit | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1001_temporary_certificate_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1001_temporary_certificate_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_user_1002_identity_enrollment | PASS | `ok` | ok |
| wrong_kind_user_1002_identity_enrollment_commit | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1002_identity_enrollment_key_certificate | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1002_identity_enrollment_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_user_1002_persistent_certificate | PASS | `ok` | ok |
| wrong_kind_user_1002_persistent_certificate_commit | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1002_persistent_certificate_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1002_persistent_certificate_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_user_1002_temporary_certificate | PASS | `ok` | ok |
| wrong_kind_user_1002_temporary_certificate_commit | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1002_temporary_certificate_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1002_temporary_certificate_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_user_1001_retired_certificate | PASS | `ok` | ok |
| wrong_kind_user_1001_retired_certificate_commit | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1001_retired_certificate_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1001_retired_certificate_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_user_1001_retired_invalidation | PASS | `ok` | ok |
| wrong_kind_user_1001_retired_invalidation_commit | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1001_retired_invalidation_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_user_1001_retired_invalidation_key_certificate | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_commit_genesis | PASS | `ok` | ok |
| wrong_kind_commit_genesis_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_genesis_key_certificate | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_genesis_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_commit_revision | PASS | `ok` | ok |
| wrong_kind_commit_revision_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_revision_key_certificate | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_revision_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_commit_temporary_session | PASS | `ok` | ok |
| wrong_kind_commit_temporary_session_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_temporary_session_key_certificate | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_temporary_session_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_commit_temporary_one_shot | PASS | `ok` | ok |
| wrong_kind_commit_temporary_one_shot_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_temporary_one_shot_key_certificate | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_temporary_one_shot_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_commit_merge | PASS | `ok` | ok |
| wrong_kind_commit_merge_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_merge_key_certificate | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_merge_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_commit_revocation | PASS | `ok` | ok |
| wrong_kind_commit_revocation_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_revocation_key_certificate | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_revocation_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_commit_restore | PASS | `ok` | ok |
| wrong_kind_commit_restore_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_restore_key_certificate | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_restore_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_commit_identity_transition | PASS | `ok` | ok |
| wrong_kind_commit_identity_transition_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_identity_transition_key_certificate | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_identity_transition_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_commit_user_1002_genesis | PASS | `ok` | ok |
| wrong_kind_commit_user_1002_genesis_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_user_1002_genesis_key_certificate | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_user_1002_genesis_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| published_positive_commit_boundary_max_size | PASS | `ok` | ok |
| wrong_kind_commit_boundary_max_size_identity_enrollment | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_boundary_max_size_key_certificate | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| wrong_kind_commit_boundary_max_size_key_invalidation | PASS | `err envelope wrong_object_type` | err envelope wrong_object_type |
| root_array_depth_7 | PASS | `err envelope not_signed_object` | err envelope not_signed_object |
| payload_array_depth_7 | PASS | `err schema wrong_type` | err schema wrong_type |
| root_map_depth_7 | PASS | `err envelope not_signed_object` | err envelope not_signed_object |
| payload_map_depth_7 | PASS | `err schema unknown_field` | err schema unknown_field |
| root_empty_array_depth_8 | PASS | `err envelope not_signed_object` | err envelope not_signed_object |
| payload_empty_array_depth_8 | PASS | `err schema wrong_type` | err schema wrong_type |
| root_empty_map_depth_8 | PASS | `err envelope not_signed_object` | err envelope not_signed_object |
| payload_empty_map_depth_8 | PASS | `err schema unknown_field` | err schema unknown_field |
| root_array_depth_8 | PASS | `err envelope not_signed_object` | err envelope not_signed_object |
| payload_array_depth_8 | PASS | `err schema wrong_type` | err schema wrong_type |
| root_map_depth_8 | PASS | `err envelope not_signed_object` | err envelope not_signed_object |
| payload_map_depth_8 | PASS | `err schema unknown_field` | err schema unknown_field |
| root_empty_array_depth_9 | PASS | `err envelope_decode depth_exceeded` | err envelope_decode depth_exceeded |
| payload_empty_array_depth_9 | PASS | `err payload_decode depth_exceeded` | err payload_decode depth_exceeded |
| root_empty_map_depth_9 | PASS | `err envelope_decode depth_exceeded` | err envelope_decode depth_exceeded |
| payload_empty_map_depth_9 | PASS | `err payload_decode depth_exceeded` | err payload_decode depth_exceeded |
| root_array_depth_9 | PASS | `err envelope_decode depth_exceeded` | err envelope_decode depth_exceeded |
| payload_array_depth_9 | PASS | `err payload_decode depth_exceeded` | err payload_decode depth_exceeded |
| root_map_depth_9 | PASS | `err envelope_decode depth_exceeded` | err envelope_decode depth_exceeded |
| payload_map_depth_9 | PASS | `err payload_decode depth_exceeded` | err payload_decode depth_exceeded |
| root_empty_array_depth_10 | PASS | `err envelope_decode depth_exceeded` | err envelope_decode depth_exceeded |
| payload_empty_array_depth_10 | PASS | `err payload_decode depth_exceeded` | err payload_decode depth_exceeded |
| root_empty_map_depth_10 | PASS | `err envelope_decode depth_exceeded` | err envelope_decode depth_exceeded |
| payload_empty_map_depth_10 | PASS | `err payload_decode depth_exceeded` | err payload_decode depth_exceeded |
| array_items_0 | PASS | `err schema wrong_type` | err schema wrong_type |
| map_pairs_0 | PASS | `err schema missing_field` | err schema missing_field |
| array_items_1 | PASS | `err schema wrong_type` | err schema wrong_type |
| map_pairs_1 | PASS | `err schema unknown_field` | err schema unknown_field |
| array_items_255 | PASS | `err schema wrong_type` | err schema wrong_type |
| map_pairs_255 | PASS | `err schema unknown_field` | err schema unknown_field |
| array_items_256 | PASS | `err schema wrong_type` | err schema wrong_type |
| map_pairs_256 | PASS | `err schema unknown_field` | err schema unknown_field |
| array_items_257 | PASS | `err payload_decode container_too_large` | err payload_decode container_too_large |
| map_pairs_257 | PASS | `err payload_decode container_too_large` | err payload_decode container_too_large |
| payload_text_1023 | PASS | `err schema wrong_type` | err schema wrong_type |
| message_bytes_1023 | PASS | `ok` | ok |
| payload_text_1024 | PASS | `err schema wrong_type` | err schema wrong_type |
| message_bytes_1024 | PASS | `ok` | ok |
| payload_text_1025 | PASS | `err payload_decode text_too_long` | err payload_decode text_too_long |
| message_bytes_1025 | PASS | `err payload_decode text_too_long` | err payload_decode text_too_long |
| head_only_3_1025 | PASS | `err payload_decode text_too_long` | err payload_decode text_too_long |
| head_only_3_61441 | PASS | `err payload_decode text_too_long` | err payload_decode text_too_long |
| head_only_2_61441 | PASS | `err payload_decode input_too_large` | err payload_decode input_too_large |
| head_only_2_61440 | PASS | `err payload_decode truncated` | err payload_decode truncated |
| head_only_3_1024 | PASS | `err payload_decode truncated` | err payload_decode truncated |
| head_only_4_256 | PASS | `err payload_decode truncated` | err payload_decode truncated |
| head_only_5_256 | PASS | `err payload_decode truncated` | err payload_decode truncated |
| object_61440 | PASS | `ok` | ok |
| object_61441 | PASS | `err envelope_decode input_too_large` | err envelope_decode input_too_large |
| reserved_0_28 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_0_29 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_0_30 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_0_31 | PASS | `err payload_decode indefinite_length` | err payload_decode indefinite_length |
| reserved_1_28 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_1_29 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_1_30 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_1_31 | PASS | `err payload_decode indefinite_length` | err payload_decode indefinite_length |
| reserved_2_28 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_2_29 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_2_30 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_2_31 | PASS | `err payload_decode indefinite_length` | err payload_decode indefinite_length |
| reserved_3_28 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_3_29 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_3_30 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_3_31 | PASS | `err payload_decode indefinite_length` | err payload_decode indefinite_length |
| reserved_4_28 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_4_29 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_4_30 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_4_31 | PASS | `err payload_decode indefinite_length` | err payload_decode indefinite_length |
| reserved_5_28 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_5_29 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_5_30 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_5_31 | PASS | `err payload_decode indefinite_length` | err payload_decode indefinite_length |
| reserved_6_28 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_6_29 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_6_30 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_6_31 | PASS | `err payload_decode indefinite_length` | err payload_decode indefinite_length |
| reserved_7_28 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_7_29 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_7_30 | PASS | `err payload_decode reserved_additional_info` | err payload_decode reserved_additional_info |
| reserved_7_31 | PASS | `err payload_decode indefinite_length` | err payload_decode indefinite_length |
| raw_1800 | PASS | `err payload_decode non_minimal_encoding` | err payload_decode non_minimal_encoding |
| raw_5800 | PASS | `err payload_decode non_minimal_encoding` | err payload_decode non_minimal_encoding |
| raw_7800 | PASS | `err payload_decode non_minimal_encoding` | err payload_decode non_minimal_encoding |
| raw_9800 | PASS | `err payload_decode non_minimal_encoding` | err payload_decode non_minimal_encoding |
| raw_b800 | PASS | `err payload_decode non_minimal_encoding` | err payload_decode non_minimal_encoding |
| raw_d812 | PASS | `err payload_decode non_minimal_encoding` | err payload_decode non_minimal_encoding |
| raw_3b8000000000000000 | PASS | `err payload_decode integer_out_of_range` | err payload_decode integer_out_of_range |
| raw_62c0af | PASS | `err payload_decode invalid_utf8` | err payload_decode invalid_utf8 |
| raw_63eda080 | PASS | `err payload_decode invalid_utf8` | err payload_decode invalid_utf8 |
| raw_64f4908080 | PASS | `err payload_decode invalid_utf8` | err payload_decode invalid_utf8 |
| raw_d280 | PASS | `err payload_decode tag_forbidden` | err payload_decode tag_forbidden |
| raw_c000 | PASS | `err payload_decode tag_forbidden` | err payload_decode tag_forbidden |
| raw_0000 | PASS | `err payload_decode trailing_bytes` | err payload_decode trailing_bytes |
| raw_f90000 | PASS | `err payload_decode float_forbidden` | err payload_decode float_forbidden |
| raw_f6 | PASS | `err payload_decode simple_value_forbidden` | err payload_decode simple_value_forbidden |
| raw_f8 | PASS | `err payload_decode truncated` | err payload_decode truncated |
| nonpreferred_simple_f814 | PASS | `err payload_decode simple_value_forbidden` | Forbidden extended simple value; rejection appropriate |
| nonpreferred_simple_f815 | PASS | `err payload_decode simple_value_forbidden` | Forbidden extended simple value; rejection appropriate |
| unknown_key_major_0 | PASS | `err schema unknown_field` | err schema unknown_field |
| unknown_key_major_1 | PASS | `err schema unknown_field` | err schema unknown_field |
| unknown_key_major_2 | PASS | `err payload_decode map_key_type` | err payload_decode map_key_type |
| unknown_key_major_3 | PASS | `err schema unknown_field` | err schema unknown_field |
| unknown_key_major_4 | PASS | `err payload_decode map_key_type` | err payload_decode map_key_type |
| unknown_key_major_5 | PASS | `err payload_decode map_key_type` | err payload_decode map_key_type |
| unknown_key_major_6 | PASS | `err payload_decode tag_forbidden` | err payload_decode tag_forbidden |
| unknown_key_major_7 | PASS | `err payload_decode map_key_type` | err payload_decode map_key_type |
| key_mix_or_tie_a20000617800 | PASS | `err payload_decode map_key_type` | err payload_decode map_key_type |
| key_mix_or_tie_a200000000 | PASS | `err payload_decode unsorted_or_duplicate_key` | err payload_decode unsorted_or_duplicate_key |
| typ_'x' | FAIL | `err envelope protected_header_invalid` | err envelope wrong_object_type |
| typ_'application/vnd.cham3.ecr-commit.v2+cose' | FAIL | `err envelope protected_header_invalid` | err envelope wrong_object_type |
| typ_'' | FAIL | `err envelope protected_header_invalid` | err envelope wrong_object_type |
| typ_16 | PASS | `err envelope protected_header_invalid` | err envelope protected_header_invalid |
| typ_False | PASS | `err envelope protected_header_invalid` | err envelope protected_header_invalid |
| typ_b'x' | PASS | `err envelope protected_header_invalid` | err envelope protected_header_invalid |
| alg_-8 | PASS | `err envelope unsupported_algorithm` | err envelope unsupported_algorithm |
| alg_-7 | PASS | `err envelope unsupported_algorithm` | err envelope unsupported_algorithm |
| alg_0 | PASS | `err envelope unsupported_algorithm` | err envelope unsupported_algorithm |
| alg_1 | PASS | `err envelope unsupported_algorithm` | err envelope unsupported_algorithm |
| alg_False | PASS | `err envelope protected_header_invalid` | err envelope protected_header_invalid |
| alg_'Ed25519' | PASS | `err envelope protected_header_invalid` | err envelope protected_header_invalid |
| alg_b'Ed25519' | PASS | `err envelope protected_header_invalid` | err envelope protected_header_invalid |
| wrong_external_aad | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_signature_before_bad_payload | PASS | `err signature signature_invalid` | err signature signature_invalid |
| protected_decode_before_bad_unprotected_kind | FAIL | `err envelope not_signed_object` | err envelope_decode non_minimal_encoding |
| nonempty_unprotected | PASS | `err envelope unprotected_not_empty` | err envelope unprotected_not_empty |
| signature_length_0 | PASS | `err envelope signature_length` | err envelope signature_length |
| signature_length_63 | PASS | `err envelope signature_length` | err envelope signature_length |
| signature_length_65 | PASS | `err envelope signature_length` | err envelope signature_length |
| scalar_7237005577332262213973186563042994240857116359379907606001950938285454250989 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| scalar_7237005577332262213973186563042994240857116359379907606001950938285454250990 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| scalar_115792089237316195423570985008687907853269984665640564039457584007913129639935 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| scalar_8597286612725651449113122106546331932615839190884420706332948831533403006688 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_R_0 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_A_0 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_R_1 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_A_1 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_R_2 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_A_2 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_R_3 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_A_3 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_R_4 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_A_4 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_R_5 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_A_5 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_R_6 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| bad_A_6 | PASS | `err signature signature_invalid` | err signature signature_invalid |
| time_'2000-01-01T00:00:00Z' | PASS | `ok` | ok |
| time_'2000-02-29T23:59:59Z' | PASS | `ok` | ok |
| time_'2400-02-29T00:00:00Z' | PASS | `ok` | ok |
| time_'9999-12-31T23:59:59Z' | PASS | `ok` | ok |
| time_'1900-02-28T00:00:00Z' | PASS | `err schema invalid_time` | err schema invalid_time |
| time_'1999-12-31T23:59:59Z' | PASS | `err schema invalid_time` | err schema invalid_time |
| time_'2100-02-29T00:00:00Z' | PASS | `err schema invalid_time` | err schema invalid_time |
| time_'2026-04-31T00:00:00Z' | PASS | `err schema invalid_time` | err schema invalid_time |
| time_'2026-00-01T00:00:00Z' | PASS | `err schema invalid_time` | err schema invalid_time |
| time_'2026-01-00T00:00:00Z' | PASS | `err schema invalid_time` | err schema invalid_time |
| time_'2026-01-01T24:00:00Z' | PASS | `err schema invalid_time` | err schema invalid_time |
| time_'2026-01-01T00:60:00Z' | PASS | `err schema invalid_time` | err schema invalid_time |
| time_'2026-01-01T00:00:60Z' | PASS | `err schema invalid_time` | err schema invalid_time |
| time_'2026-01-01t00:00:00Z' | PASS | `err schema invalid_time` | err schema invalid_time |
| time_'2026-01-01T00:00:00z' | PASS | `err schema invalid_time` | err schema invalid_time |
| time_'2026-01-01T00:00:00Z\n' | PASS | `err schema invalid_time` | err schema invalid_time |
| time_'٢٠٢٦-01-01T00:00:00Z' | PASS | `err schema invalid_time` | err schema invalid_time |
| time_'２０２６-01-01T00:00:00Z' | PASS | `err schema invalid_time` | err schema invalid_time |
| message_edge_0xa0 | PASS | `err schema invalid_text` | err schema invalid_text |
| message_edge_0x1680 | PASS | `err schema invalid_text` | err schema invalid_text |
| message_edge_0x2000 | PASS | `err schema invalid_text` | err schema invalid_text |
| message_edge_0x202f | PASS | `err schema invalid_text` | err schema invalid_text |
| message_edge_0x205f | PASS | `err schema invalid_text` | err schema invalid_text |
| message_edge_0x3000 | PASS | `err schema invalid_text` | err schema invalid_text |
| message_edge_0x85 | PASS | `err schema invalid_text` | err schema invalid_text |
| message_edge_0xfeff | PASS | `err schema invalid_text` | err schema invalid_text |
| message_nonwhitespace_0x200b | PASS | `ok` | ok |
| message_nonwhitespace_0x2060 | PASS | `ok` | ok |
| message_lines_16 | PASS | `ok` | ok |
| message_lines_17 | PASS | `err schema invalid_text` | err schema invalid_text |
| label_order_['aa', 'b'] | BLOCKED | `err schema list_not_strictly_ascending` | R04 comparator unresolved |
| label_order_['b', 'aa'] | BLOCKED | `ok` | R04 comparator unresolved |
| label_order_['a', 'a'] | PASS | `err schema list_not_strictly_ascending` | err schema list_not_strictly_ascending |
| label_order_['aa', 'ab'] | PASS | `ok` | Ascending under both comparison readings |
| constraints_length_32 | PASS | `ok` | ok |
| constraints_length_33 | FAIL | `err schema list_length` | err schema list_too_long |
| operation_ties_or_length_2 | PASS | `err schema list_not_strictly_ascending` | err schema list_not_strictly_ascending |
| operation_ties_or_length_0 | BLOCKED | `err schema list_length` | Minimum-one violation rejects; E1 code conflict |
| operation_ties_or_length_33 | FAIL | `err schema list_length` | err schema list_too_long |
| expiration_equal | PASS | `err intrinsic lifetime_rule` | err intrinsic lifetime_rule |
| expiration_earlier | PASS | `err intrinsic lifetime_rule` | err intrinsic lifetime_rule |
| expiration_later | PASS | `ok` | ok |
| certificate_bad_subject_point_0 | PASS | `ok` | Observed validity only; identityd must reject invalid subject at certification |
| certificate_bad_subject_point_1 | PASS | `ok` | Observed validity only; identityd must reject invalid subject at certification |
| certificate_bad_subject_point_2 | PASS | `ok` | Observed validity only; identityd must reject invalid subject at certification |
| certificate_bad_subject_point_3 | PASS | `ok` | Observed validity only; identityd must reject invalid subject at certification |

## Appendix C — exact reviewer reproduction scripts

Create the following local files under the evidence mount before executing the commands above. These are review/test code included in this document, not production changes. `adversarial.py` uses its own deterministic encoder and frozen synthetic key seeds. Only decoding the frozen starting vectors and obtaining comparison answers uses the existing oracle. Expected normative outcomes are assigned independently.

### adversarial.py

SHA-256: `8d04c0e3239860f66afbd02acde393f557091038442b1c7dded8f74a7b143e44`.

```python
"""Independent review inputs. Only published synthetic keys are used.

Run inside the pinned candidate image, with candidate directory as cwd:
python3 /evidence/own/adversarial.py PROBE /evidence/own/results.json
Expected codes are specified below independently of oracle output.
"""
import copy
import hashlib
import json
import os
import subprocess
import sys

sys.path.insert(0, os.path.join(os.getcwd(), 'tests/m2/oracle'))
import ecr_oracle as oracle
from nacl.signing import SigningKey


def head(major, n):
    for width, additional, limit in [(0, n, 24), (1, 24, 256), (2, 25, 65536),
                                     (4, 26, 2**32), (8, 27, 2**64)]:
        if n < limit:
            return bytes([major * 32 + additional]) + (n.to_bytes(width, 'big') if width else b'')
    raise ValueError(n)


def encode(value):
    if isinstance(value, bool):
        return b'\xf5' if value else b'\xf4'
    if isinstance(value, int):
        return head(0, value) if value >= 0 else head(1, -1-value)
    if isinstance(value, bytes):
        return head(2, len(value)) + value
    if isinstance(value, str):
        raw = value.encode('utf-8')
        return head(3, len(raw)) + raw
    if isinstance(value, list):
        return head(4, len(value)) + b''.join(map(encode, value))
    if isinstance(value, dict):
        return head(5, len(value)) + b''.join(k+v for k, v in sorted((encode(k), encode(v)) for k, v in value.items()))
    raise TypeError(type(value))


with open('tests/m2/vectors/ecr_v1/vectors.json') as handle:
    document = json.load(handle)
keys = {bytes.fromhex(k['key_id']): (bytes.fromhex(k['public_key']), SigningKey(bytes.fromhex(k['seed'])))
        for k in document['keys']}
resolver = oracle.resolver_t([])
resolver.keys = {kid: key[0] for kid, key in keys.items()}
objects = {}
for vector in document['valid']:
    raw = bytes.fromhex(vector['envelope'])
    root = oracle.decoder_t(raw, 'envelope_decode').decode_top(True)
    protected, _, payload, signature = root.value
    header = oracle.decoder_t(protected, 'envelope_decode').decode_top()
    decoded = oracle.decoder_t(payload, 'payload_decode').decode_top()
    objects[vector['name']] = (vector['kind'], header[4], header, decoded, raw)
genesis = objects['commit_genesis']
kind, kid, header, payload, original = genesis
aad = b'cham3/ecr-commit/v1'
cases = []


def envelope(protected, payload_bytes, signature, unprotected=None):
    return b'\xd2' + encode([protected, {} if unprotected is None else unprotected, payload_bytes, signature])


def signed(raw_payload, edited_header=None, edited_aad=None):
    protected = encode(header if edited_header is None else edited_header)
    message = encode(['Signature1', protected, aad if edited_aad is None else edited_aad, raw_payload])
    return envelope(protected, raw_payload, keys[kid][1].sign(message).signature)


def add(name, data, expected=None, expected_kind='commit', key_override=None, note=''):
    cases.append(dict(name=name, data=data, spec_expected=expected, kind=expected_kind,
                      key_override=key_override, note=note))


def edit(name, path, replacement, expected=None, note=''):
    changed = copy.deepcopy(payload)
    node = changed
    for part in path[:-1]:
        node = node[part]
    node[path[-1]] = replacement
    add(name, signed(encode(changed)), expected, note=note)


add('positive_genesis', original, 'ok')
for name, obj in objects.items():
    add('published_positive_'+name, obj[4], 'ok', obj[0])
    for other in ['commit', 'identity_enrollment', 'key_certificate', 'key_invalidation']:
        if other != obj[0]:
            add('wrong_kind_'+name+'_'+other, obj[4], 'err envelope wrong_object_type', other)

# Raw decoder boundaries, on the envelope and on authenticated payloads.
for depth in [7, 8, 9]:
    for family, prefix, tail in [('array', b'\x81', b'\x00'), ('map', b'\xa1\x00', b'\x00'),
                                 ('empty_array', b'\x81', b'\x80'), ('empty_map', b'\xa1\x00', b'\xa0')]:
        raw = prefix * depth + tail
        actual_depth = depth + (1 if family.startswith('empty') else 0)
        decode_code = 'depth_exceeded' if actual_depth > 8 else None
        add('root_'+family+'_depth_'+str(actual_depth), raw,
            'err envelope_decode '+decode_code if decode_code else 'err envelope not_signed_object')
        add('payload_'+family+'_depth_'+str(actual_depth), signed(raw),
            'err payload_decode '+decode_code if decode_code else 'err schema '+('unknown_field' if 'map' in family else 'wrong_type'))
for n in [0, 1, 255, 256, 257]:
    raw = head(4, n) + b'\x00'*n
    add('array_items_'+str(n), signed(raw), 'err payload_decode container_too_large' if n > 256 else 'err schema wrong_type')
    raw = head(5, n) + b''.join(encode(i)+b'\x00' for i in range(n))
    add('map_pairs_'+str(n), signed(raw), 'err payload_decode container_too_large' if n > 256 else
        'err schema '+('missing_field' if n == 0 else 'unknown_field'))
for n in [1023, 1024, 1025]:
    add('payload_text_'+str(n), signed(encode('x'*n)), 'err payload_decode text_too_long' if n > 1024 else 'err schema wrong_type')
    edit('message_bytes_'+str(n), ['message'], 'x'*n, 'err payload_decode text_too_long' if n > 1024 else 'ok')
for major, n, code in [(3,1025,'text_too_long'), (3,61441,'text_too_long'),
                        (2,61441,'input_too_large'), (2,61440,'truncated'),
                        (3,1024,'truncated'), (4,256,'truncated'), (5,256,'truncated')]:
    add('head_only_'+str(major)+'_'+str(n), signed(head(major,n)), 'err payload_decode '+code,
        note='Remaining-input precedence and allocation inspected separately.')
maximum = max(objects.values(), key=lambda obj: len(obj[4]))
assert len(maximum[4]) == 61440
add('object_61440', maximum[4], 'ok', maximum[0])
add('object_61441', maximum[4]+b'\x00', 'err envelope_decode input_too_large', maximum[0])
for major in range(8):
    for additional in [28,29,30,31]:
        code = 'indefinite_length' if additional==31 else 'reserved_additional_info'
        add('reserved_'+str(major)+'_'+str(additional), signed(bytes([major*32+additional])), 'err payload_decode '+code)
for raw, code in [(b'\x18\x00','non_minimal_encoding'), (b'\x58\x00','non_minimal_encoding'),
                   (b'\x78\x00','non_minimal_encoding'), (b'\x98\x00','non_minimal_encoding'),
                   (b'\xb8\x00','non_minimal_encoding'), (b'\xd8\x12','non_minimal_encoding'),
                   (b'\x3b\x80'+b'\x00'*7,'integer_out_of_range'),
                   (b'\x62\xc0\xaf','invalid_utf8'), (b'\x63\xed\xa0\x80','invalid_utf8'),
                   (b'\x64\xf4\x90\x80\x80','invalid_utf8'),
                   (b'\xd2\x80','tag_forbidden'), (b'\xc0\x00','tag_forbidden'),
                   (b'\x00\x00','trailing_bytes'), (b'\xf9\x00\x00','float_forbidden'),
                   (b'\xf6','simple_value_forbidden'), (b'\xf8','truncated')]:
    add('raw_'+raw.hex(), signed(raw), 'err payload_decode '+code)
# Encoded booleans using a longer simple-value head are ambiguous under §2.4;
# record rather than claiming a normative error without a rule for simple args.
for raw in [b'\xf8\x14', b'\xf8\x15']:
    add('nonpreferred_simple_'+raw.hex(), signed(raw), None,
        note='§2.2 integer arguments exclude simple values; forbidden extended simple form still rejects.')

# Unknown map keys: every major type, with well-formed values to avoid masking.
for raw_key, expected in [(encode(0),'err schema unknown_field'), (encode(-1),'err schema unknown_field'),
                          (encode(b'x'),'err payload_decode map_key_type'), (encode('x'),'err schema unknown_field'),
                          (b'\x80','err payload_decode map_key_type'), (b'\xa0','err payload_decode map_key_type'),
                          (b'\xc0\x00','err payload_decode tag_forbidden'), (b'\xf4','err payload_decode map_key_type')]:
    add('unknown_key_major_'+str(raw_key[0]>>5), signed(b'\xa1'+raw_key+b'\x00'), expected)
for raw in [b'\xa2\x00\x00\x61x\x00', b'\xa2\x00\x00\x00\x00']:
    add('key_mix_or_tie_'+raw.hex(), signed(raw), 'err payload_decode '+
        ('map_key_type' if b'x' in raw else 'unsorted_or_duplicate_key'))

# Headers: unknown/future textual types are expressly wrong_object_type (§4.1/§8).
for typ in ['x', 'application/vnd.cham3.ecr-commit.v2+cose', '', 16, False, b'x']:
    changed = dict(header, **{})
    changed[16] = typ
    add('typ_'+repr(typ), signed(encode(payload), changed),
        'err envelope wrong_object_type' if isinstance(typ,str) else 'err envelope protected_header_invalid')
for alg in [-8,-7,0,1,False,'Ed25519',b'Ed25519']:
    changed = dict(header)
    changed[1] = alg
    add('alg_'+repr(alg), signed(encode(payload), changed),
        'err envelope unsupported_algorithm' if isinstance(alg,int) and not isinstance(alg,bool) else 'err envelope protected_header_invalid')
protected = encode(header)
add('wrong_external_aad', signed(encode(payload), edited_aad=b'cham3/key-certificate/v1'), 'err signature signature_invalid')
add('bad_signature_before_bad_payload', envelope(protected,b'\xff',b'\x00'*64), 'err signature signature_invalid')
add('protected_decode_before_bad_unprotected_kind', envelope(b'\x18\x00',b'\x00',b'\x00'*64,False),
    'err envelope_decode non_minimal_encoding', note='Layer 1 protected decode vs layer 2 element typing.')
add('nonempty_unprotected', envelope(protected,encode(payload),b'\x00'*64,{1:0}), 'err envelope unprotected_not_empty')
for n in [0,63,65]:
    add('signature_length_'+str(n), envelope(protected,encode(payload),b'\x00'*n), 'err envelope signature_length')

# Strict signature components and resolver keys.
message = encode(['Signature1',protected,aad,encode(payload)])
sig = keys[kid][1].sign(message).signature
order = 2**252 + 27742317777372353535851937790883648493
for scalar in [order,order+1,2**256-1,int.from_bytes(sig[32:],'little')+order]:
    add('scalar_'+str(scalar), envelope(protected,encode(payload),sig[:32]+scalar.to_bytes(32,'little')), 'err signature signature_invalid')
bad_points = [b'\x00'*32,b'\x01'+b'\x00'*31,bytes.fromhex('ec'+'ff'*30+'7f'),
              bytes.fromhex('ed'+'ff'*30+'7f'),bytes.fromhex('ee'+'ff'*30+'7f'),
              b'\x01'+b'\x00'*30+b'\x80',b'\xff'*32]
for i, point in enumerate(bad_points):
    add('bad_R_'+str(i), envelope(protected,encode(payload),point+sig[32:]), 'err signature signature_invalid')
    add('bad_A_'+str(i), original, 'err signature signature_invalid', key_override=(kid,point))

# Gregorian edge instants, exact ASCII and whitespace rules.
for time, good in [('2000-01-01T00:00:00Z',True),('2000-02-29T23:59:59Z',True),
                   ('2400-02-29T00:00:00Z',True),('9999-12-31T23:59:59Z',True),
                   ('1900-02-28T00:00:00Z',False),('1999-12-31T23:59:59Z',False),
                   ('2100-02-29T00:00:00Z',False),('2026-04-31T00:00:00Z',False),
                   ('2026-00-01T00:00:00Z',False),('2026-01-00T00:00:00Z',False),
                   ('2026-01-01T24:00:00Z',False),('2026-01-01T00:60:00Z',False),
                   ('2026-01-01T00:00:60Z',False),('2026-01-01t00:00:00Z',False),
                   ('2026-01-01T00:00:00z',False),('2026-01-01T00:00:00Z\n',False),
                   ('٢٠٢٦-01-01T00:00:00Z',False),('２０２６-01-01T00:00:00Z',False)]:
    edit('time_'+repr(time), ['timestamp'], time, 'ok' if good else 'err schema invalid_time')
for character in ['\u00a0','\u1680','\u2000','\u202f','\u205f','\u3000','\u0085','\ufeff']:
    edit('message_edge_'+hex(ord(character)), ['message'], 'x'+character, 'err schema invalid_text')
for character in ['\u200b','\u2060']:
    edit('message_nonwhitespace_'+hex(ord(character)), ['message'], 'x'+character, 'ok')
for n in [16,17]:
    edit('message_lines_'+str(n), ['message'], '\n'.join(['x']*n), 'ok' if n==16 else 'err schema invalid_text')

# Text-list order ambiguity: plain byte order and encoded length order disagree.
for items in [['aa','b'],['b','aa'],['a','a'],['aa','ab']]:
    edit('label_order_'+repr(items), ['policy','mandatory_constraints'], items,
         'err schema list_not_strictly_ascending' if items==['a','a'] else None,
         note='§6.1 never specifies encoded order for list strings; two distinguishable readings recorded.')
for n in [32,33]:
    edit('constraints_length_'+str(n), ['policy','mandatory_constraints'], ['x%02d'%i for i in range(n)],
         'ok' if n==32 else 'err schema list_too_long', note='E1 approved vector/prose conflict.')
grants = payload['delta']['add_grants']
assert grants
for operation_list in [['a','a'],[],['a']*33]:
    changed = copy.deepcopy(payload)
    for group in [changed['delta']['add_grants'], changed['result_state']['grants']]:
        group[0]['resource']['operations'] = operation_list
    add('operation_ties_or_length_'+str(len(operation_list)), signed(encode(changed)),
        'err schema list_not_strictly_ascending' if len(operation_list)==2 else
        'err schema list_too_long' if len(operation_list)==33 else None, note='Empty operations must reject; E1 code unresolved.')
for relation in ['equal','earlier','later']:
    changed = copy.deepcopy(payload)
    for group in [changed['delta']['add_grants'], changed['result_state']['grants']]:
        start = group[0]['lifetime']['effective_start']
        group[0]['lifetime']['expiration'] = start if relation=='equal' else '2000-01-01T00:00:00Z' if relation=='earlier' else '9999-12-31T23:59:59Z'
    add('expiration_'+relation, signed(encode(changed)), 'ok' if relation=='later' else 'err intrinsic lifetime_rule')

# Certificate key-point validity is explicitly handed to M2-004 in the handoff;
# verify and preserve the actual layers 1-5 behavior without claiming authority.
certificate = next(obj for obj in objects.values() if obj[0]=='key_certificate')
for i, point in enumerate(bad_points[:4]):
    cert_payload = copy.deepcopy(certificate[3])
    cert_payload['subject_public_key'] = point
    cert_payload['subject_key_id'] = hashlib.sha256(b'cham3/key-id/ed25519/v1\x00'+point).digest()
    cert_protected = encode(certificate[2])
    cert_raw = encode(cert_payload)
    cert_message = encode(['Signature1',cert_protected,b'cham3/key-certificate/v1',cert_raw])
    cert_signature = keys[certificate[1]][1].sign(cert_message).signature
    add('certificate_bad_subject_point_'+str(i), envelope(cert_protected,cert_raw,cert_signature), None,'key_certificate',
        note='CDDL validated point / §4.3 enrollment-certification obligation; signature signer itself remains valid.')

lines = ['key %s %s'%(k.hex(),p.hex()) for k,(p,_) in keys.items()]
for case in cases:
    if case['key_override']:
        k,p = case['key_override']
        lines.append('key %s %s'%(k.hex(),p.hex()))
    lines.append('verify %s %s'%(case['kind'],case['data'].hex()))
    if case['key_override']:
        lines.append('key %s %s'%(kid.hex(),keys[kid][0].hex()))
completed = subprocess.run([sys.argv[1]],input='\n'.join(lines)+'\n',text=True,capture_output=True)
assert completed.returncode==0, completed.stderr
answers = completed.stdout.splitlines()
assert len(answers)==len(cases), (len(answers),len(cases))
records=[]
for case, answer in zip(cases,answers):
    local_resolver = oracle.resolver_t([])
    local_resolver.keys = dict(resolver.keys)
    if case['key_override']:
        k,p=case['key_override']
        local_resolver.keys[k]=p
    try:
        oracle.verify(case['data'],case['kind'],local_resolver)
        oracle_answer='ok'
    except oracle.oracle_error_t as error:
        oracle_answer='err %s %s'%(error.layer,error.code)
    normalized='ok' if answer.startswith('ok ') else answer
    records.append(dict(name=case['name'],kind=case['kind'],input_sha256=hashlib.sha256(case['data']).hexdigest(),
                        bytes=len(case['data']),rust=normalized,oracle=oracle_answer,
                        spec_expected=case['spec_expected'],note=case['note'],
                        rust_oracle_match=normalized==oracle_answer,
                        spec_match=None if case['spec_expected'] is None else normalized==case['spec_expected']))
result=dict(cases=len(records),rust_oracle_mismatches=sum(not x['rust_oracle_match'] for x in records),
            spec_mismatches=sum(x['spec_match'] is False for x in records),
            judgments_pending=sum(x['spec_match'] is None for x in records),records=records)
with open(sys.argv[2],'w') as handle:
    json.dump(result,handle,indent=2)
print(json.dumps({k:v for k,v in result.items() if k!='records'}))
for record in records:
    if record['spec_match'] is False or record['spec_match'] is None or not record['rust_oracle_match']:
        print(json.dumps(record))
```

### allocations.py

SHA-256: `ed3992097b2b4b09fa7a0fa516507cbe58a58e2aebbfb5fab9eb2ce79e96c566`.

```python
import subprocess
for name,raw in [('array_256_no_items','990100'),('map_256_no_pairs','b90100')]:
    command=['valgrind','--tool=massif','--time-unit=B','--detailed-freq=1','--max-snapshots=1000',
             '--threshold=0.0','--massif-out-file=/evidence/own/massif_'+name+'.txt',
             '/evidence/own/target/debug/cham3_format_probe']
    done=subprocess.run(command,input='verify commit '+raw+'\n',capture_output=True,text=True)
    assert done.returncode==0,(name,done.stderr)
    assert done.stdout=='err envelope_decode truncated\n',(name,done.stdout)
    with open('/evidence/own/allocation_'+name+'.log','w') as handle:
        handle.write(done.stdout+done.stderr)
    print(name,done.stdout.strip())
```

### defects.py

SHA-256: `06cbf1f86988147a907e8c612fc1d45aefa41260d16b3d871cf26df5b53e15ba`.

```python
"""Disposable sensitivity checks, with per-defect records retained for review."""
import json
import os
import shutil
import subprocess
import sys

root = os.getcwd()
sys.path.insert(0, root+'/tests/m2/harness')
from test_m2_format_verifier import SeededDefects
import m2_harness
directory='/evidence/own/defect_workspace'
os.makedirs(directory,exist_ok=False)
for name in ['Cargo.toml','Cargo.lock']:
    shutil.copy2(root+'/'+name,directory+'/'+name)
for name in ['core','.cargo']:
    shutil.copytree(root+'/'+name,directory+'/'+name)
environment=dict(os.environ,CARGO_TARGET_DIR=directory+'/target')
environment.pop('RUSTFLAGS',None)
results=[]
own_defect=('expiration_equality_accepted','core/contract_format/src/intrinsic.rs',
            '*expiration <= lifetime.effective_start','*expiration < lifetime.effective_start')
for name,path,anchor,replacement in [('unmodified','','','')]+SeededDefects+[own_defect]:
    if path:
        target=directory+'/'+path
        with open(target) as handle:
            source=handle.read()
        assert source.count(anchor)==1,(name,source.count(anchor))
        with open(target,'w') as handle:
            handle.write(source.replace(anchor,replacement))
    try:
        built=subprocess.run(['cargo','build','--frozen','-p','cham3_contract_format','--bin','cham3_format_probe'],
                             cwd=directory,env=environment,capture_output=True,text=True)
        with open('/evidence/own/defect_build_'+name+'.log','w') as handle:
            handle.write(built.stdout+built.stderr)
        assert built.returncode==0,(name,built.stderr)
        run_dir='/evidence/own/defect_'+name
        os.makedirs(run_dir)
        record=m2_harness.run_format_differential(directory+'/target/debug/cham3_format_probe',run_dir,3,300)
        if name=='expiration_equality_accepted':
            # Our own corpus has a discriminating signed equality case.
            run=subprocess.run(['python3','/evidence/own/adversarial.py',directory+'/target/debug/cham3_format_probe',
                                '/evidence/own/defect_own_adversarial.json'],cwd=root,capture_output=True,text=True)
            assert run.returncode==0,run.stderr
            with open('/evidence/own/defect_own_adversarial.json') as handle:
                own=json.load(handle)
            equality=next(r for r in own['records'] if r['name']=='expiration_equal')
            record['own_case']=equality
            assert equality['rust']=='ok' and equality['oracle']=='err intrinsic lifetime_rule'
        else:
            assert record['result']==('PASS' if name=='unmodified' else 'FAIL'),name
            assert name=='unmodified' or record['mismatch_count']>0,name
        results.append(dict(name=name,result=record['result'],mismatch_count=record['mismatch_count'],
                            own_case=record.get('own_case')))
        print(json.dumps(results[-1]),flush=True)
        with open('/evidence/own/defects.json','w') as handle:
            json.dump(results,handle,indent=2)
    finally:
        if path:
            with open(target,'w') as handle:
                handle.write(source)
```

### oracle_reproduction.py

SHA-256: `74d20f7b1c64ae7d42b150403fc04779ef8798af934c9b25b4cad836b721c577`.

```python
import importlib.util
import json
import os
import subprocess
import sys

sys.path.insert(0,os.getcwd()+'/tests/m2/oracle')
import format_differential as differential
import ecr_oracle as fixed
spec=importlib.util.spec_from_file_location('oracle_before','/evidence/own/oracle_before.py')
before=importlib.util.module_from_spec(spec)
spec.loader.exec_module(before)
differential.ecr_oracle=before
cases,keys=differential.build_corpus('tests/m2/vectors/ecr_v1/vectors.json',seed=3,cases_per_object=300)
probe='/evidence/own/target/debug/cham3_format_probe'
input_text=differential.probe_input(cases,keys)
completed=subprocess.run([probe],input=input_text,capture_output=True,text=True)
assert completed.returncode==0,completed.stderr
answers=completed.stdout.splitlines()
mismatches=[]
for index,(case,actual) in enumerate(zip(cases,answers)):
    if actual!=case['expected']:
        mismatches.append(dict(index=index,family=case['family'],kind=case['kind'],input_sha256=__import__('hashlib').sha256(case['data']).hexdigest(),expected=case['expected'],actual=actual))
record=dict(cases=len(cases),mismatch_count=len(mismatches),mismatches=mismatches,
            corpus_sha256=__import__('hashlib').sha256(input_text.encode('ascii')).hexdigest())
assert len(mismatches)==4,record
with open('/evidence/own/oracle_before_reproduced.json','w') as handle:
    json.dump(record,handle,indent=2)
print(json.dumps(record,indent=2))
```

## Appendix D — complete independently reproduced pre-fix mismatch record

```json
{
  "cases": 5642,
  "mismatch_count": 4,
  "mismatches": [
    {
      "index": 1764,
      "family": "structure",
      "kind": "key_certificate",
      "input_sha256": "42b8722a453590bcbb1f9e73cfd66a11e7d2338c8e3a4221d953f9062c0afe0b",
      "expected": "ok 42b8722a453590bcbb1f9e73cfd66a11e7d2338c8e3a4221d953f9062c0afe0b same",
      "actual": "err schema invalid_time"
    },
    {
      "index": 2034,
      "family": "structure",
      "kind": "key_certificate",
      "input_sha256": "5b4cc93a6661b62dbe9e326be1ee8dff155f3cd9b473bcb6d5e5a031dffb094a",
      "expected": "ok 5b4cc93a6661b62dbe9e326be1ee8dff155f3cd9b473bcb6d5e5a031dffb094a same",
      "actual": "err schema invalid_time"
    },
    {
      "index": 2429,
      "family": "structure",
      "kind": "commit",
      "input_sha256": "2769bcf1319975b38a83f49012eda5e784611346ddf7449bcd65aea22e33e94c",
      "expected": "err intrinsic result_state_mismatch",
      "actual": "err schema invalid_time"
    },
    {
      "index": 2487,
      "family": "structure",
      "kind": "commit",
      "input_sha256": "a105755c4c65f60420461a6ef11e8dea34ece29a41168f0db1cb0ca81d2c4275",
      "expected": "err schema list_not_strictly_ascending",
      "actual": "err schema invalid_text"
    }
  ],
  "corpus_sha256": "b714ad731b3052d822b3046d1c3cca350f171c69788a5b42f6dabb5ff6d6477f"
}
```
