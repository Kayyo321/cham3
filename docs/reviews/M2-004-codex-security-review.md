# M2-004 independent Codex review

**Verdict: CHANGES_REQUESTED.** Five findings require correction and an exact-commit re-review. The supplied qualification passes; it does not cover the failure sequences reproduced below. No implementation files were edited, no implementation or review branch was merged, and no owner architecture or risk decision is made here.

Reviewer: Codex, independent REVIEWER, 2026-10-13 UTC. Review branch: `review/m2-004-security`. Publication base: `9cf882503848f8c6cc4938a588cc9fc062a9392d` (fetched origin/main). The publication adds only this record. The candidate remains a separate detached, clean clone.

## Exact targets and environment

- Baseline: `20c849b22ff7cb56c9266d447a28d6c692cbe532`.
- Candidate: `bb3f1bad77080da34069af62c8d2cc390f2fcf7e`; tree `e404c02385ab7de273841ac5d796fb5414e90af2`.
- Implementation commits: `0e4f993172019de8a55badbdd8a0a56943b89eb1`, `e5be2065d10c1f41bac27020540117175ff46d5b`, `bb3f1bad77080da34069af62c8d2cc390f2fcf7e`.
- Docs follow-up: `d2c29ad7a09b2b1d49a23e9fa92c9656aa74c711`. **PASS: docs-only.** `git diff --name-status bb3f1ba..origin/codex/m2-cryptographic-contract-core` and `git diff --numstat bb3f1ba..d2c29ad` contain only `docs/` paths. A final `git ls-remote --heads origin codex/m2-cryptographic-contract-core` still returned d2c29ad.
- Clean clone: `/workspace/m2-004-review`, owner UID/GID 1000, mode 0700; `git clone https://github.com/Kayyo321/cham3.git /workspace/m2-004-review`, then `git checkout --detach bb3f1bad77080da34069af62c8d2cc390f2fcf7e`.
- Outer machine: managed cloud container `672e8a5ec7e4`, Debian 13.6, Linux 6.18.44, x86_64, UID/GID 1000 (agent). Docker client/server 28.4.0, vfs storage, runc, seccomp enabled.
- Built image: `cham3-m2-dev`, `sha256:f5325b06d9a13a462c8cf47058eaa045155b388a9b834d12a92ff860a09631e3`, linux/amd64. Pinned base index `debian@sha256:913f6706df59a68922d1dd08f78c2476560a8d367897200a6005b00e5f67c2d5`.
- Inside image: Debian 13.7, UID/GID 1000 (builder), rustc 1.85.1 `4eb161250e340c8f48f66e2b929ef4a5bed7c181`, LLVM 19.1.7; Cargo 1.85.1 `d73d2caf9`. Full package/deb/source locks passed.
- Network: restricted, enforced HTTP proxy policy; package hosts and github.com allowed. Git clone/push use the configured route. CLI access to api.github.com returned Forbidden. Connected GitHub tools retrieved run metadata, artifact metadata and all seven job logs. The artifact's returned File Service download URL returned HTTP 403 from this executor; no proxy or TLS bypass was attempted. No configured runtime secrets, outbound identities or VPN.
- All candidate qualification, protection and adversarial containers used `--user "$(id -u):$(id -g)"`; none used `--privileged`, added capabilities, or qualification `--record`.

Read authority at the candidate: AGENTS.md, relevant PROJECT §§3–5, 14, Phase 2, workflow/roles/orchestration/testing/security invariants; task M2-004, A04/A05/A12; ADR 0005 in full, ADR 0006 §§3–5/10, ADR 0008 §4; service IPC v1 in full and wire §7 plus supporting semantics. The handoff and its evidence were read at d2c29ad with `git show d2c29ad:docs/handoffs/M2-004-identity-vault.md`, without overlaying them on the candidate. Author evidence is corroborating material, not approval.

## Required changes

### R01 — Medium — an older invalidation retry clears a pending compromise

**FAIL.** Location: `core/identityd/src/service.rs:284` (also the unconditional removal at 295).

Sequence: persist a `rotated` invalidation; accept a different `compromised` invalidation while appends fail; observe `restriction_not_durable` and an in-memory `compromised` reason; restore appends; retry the older, already durable rotated object. The `already_present` branch removes the pending denial by target key alone. Resolution now reports `Some(rotated)`, losing the stronger `Some(compromised)` restriction.

Both reasons stop new signatures, but ADR 0005 §2 distinguishes their effect on earlier commits: compromise makes every signed commit ineligible, while rotation preserves earlier eligibility. Losing the compromise reason can therefore weaken the authorization evidence future policyd receives. This review reproduces the reason downgrade, not activation of grants in the still-unimplemented policy service.

Required: clear a pending restriction only when durable state covers that exact restriction or a stronger one. Preserve a pending compromise across weaker or unrelated retries, including the newly-appended branch, and ensure the approved ledger integration preserves it across restart. Add this sequence to service and fault tests. This also qualifies the implementation of author decision 4's escalation rule.

Reproducer: `defect_pending_compromise_is_not_cleared_by_old_rotated_retry`. Debug and release: FAIL, actual `Some(rotated)`, expected `Some(compromised)`.

### R02 — Medium — a live proposal extends a temporary key's signing window

**FAIL.** Location: `core/vaultd/src/service.rs:743`; supporting timestamp-only check in `core/identityd/src/index.rs:676`.

Prepare a temporary-session proposal five seconds before the key certificate expires, with its actual preparation timestamp inside the window. Register it normally for 120 seconds. Approve six seconds later, one second beyond `not_after`. The vault signs and the returned COSE envelope verifies cryptographically. Neither contractd compromise nor a forged timestamp is required.

The historical layer-6 check compares the immutable payload timestamp to the certificate window. That is useful for object verification, but does not establish that the key may sign **now**. The vault checks only proposal expiry after decryption, allowing this still-live proposal to extend the temporary signing authority. ADR 0005 §2 requires rejection outside the temporary certificate window.

Required: independently enforce the signing key's current validity window at proposal registration and immediately before signing, including time elapsed during KDF/decryption. Keep historical verification semantics separate. Add the boundary-crossing positive/negative test.

Reproducer: `defect_certificate_expiry_during_live_proposal_prevents_signing`. Debug and release: FAIL; actual ACCEPTED with a verified signature, expected `key_authority/certificate_window`.

### R03 — Medium — key authority can change during unlock without a final recheck

**FAIL.** Location: `core/vaultd/src/service.rs:814` and signing at 825.

The only approval-time status/scope resolution occurs before Argon2, decryption, and attempt-record handling. After unlock the code rechecks proposal expiry but does not resolve authority again. A legitimate deletion/containment/invalidation in that interval can precede the actual signature.

The deterministic adversarial scheduler publishes `deletion_pending` at the second monotonic expiry read: after unlock, before `Self::sign`. identityd then resolves the key as denied/user_deleted, yet the operation returns a cryptographically valid envelope. The scheduler stands in for another service's concurrent restriction; it is an in-process interleaving test, not a real socket race.

Required: resolve and enforce current key status and scope after unlock, immediately before signing; refuse storage/authority uncertainty and clear the seed on refusal. Test invalidation, deletion and ceremony changes during derivation. Carry the remaining cross-service serialization/linearization obligation into the already-approved M2-006/007 integration. A post-unlock sample narrows this demonstrated gap but must not be described as eliminating every distributed race. ADR 0005 §2 and ADR 0006 §5 require fresh status/scope enforcement for every signature.

Reproducer: `defect_rechecks_authority_after_unlock_before_signature`. Debug and release: FAIL; actual ACCEPTED with a verified signature while resolution is denied/user_deleted.

### R04 — Low — used proposal IDs become reusable through cleanup

**FAIL.** Location: `core/vaultd/src/service.rs:767`; also `cancel_proposal` at 849 removes used entries.

Register proposal A and approve it once. Register a different proposal B, which runs `expire_proposals` and deletes A solely because `used` is true, even while A's original lifetime is live. Re-register A with the same ID, payload and digest. Approve it again: the second envelope is byte-identical to the first.

This bypasses the stated proposal-ID single-use behavior and the `duplicate_identifier`/used checks. It still requires contractd registration and a passphrase submission for the second signing; it does not demonstrate cached-unlock signing or duplicate ECR admission. That limits severity.

Required: preserve used/canceled identifier state over the applicable proposal lifetime, reject re-registration of an already-used ID, and require a fresh ID for a fresh approval workflow. Make cancellation unable to erase the used marker. Keep retention bounded and test interleaved registrations and cancellation, not just two consecutive approve calls. Admission must independently reject duplicate request IDs in M2-005.

Reproducer: `defect_used_proposal_identifier_cannot_be_re_registered_after_gc`. Debug and release: FAIL; actual ACCEPTED, expected `ipc/duplicate_identifier`; the test verifies both returned envelopes are identical.

### R05 — Medium — a naming error erases still-nameable recovery keys without status

**FAIL.** Location: `core/vaultd/src/service.rs:1138`.

`recover_keys` reconciles only errors classified as transient. Every other registration/naming error calls `finish_bundle(uid, bundle.new_keys)` directly. An error code alone does not establish the ceremony's authoritative naming state.

The adversarial registry adapter returns `ipc/role_denied` on naming and makes recovery_status unavailable, modeling an authenticated-service access/configuration failure. Both certificates have already become durable, the epoch-2 ceremony remains open, and identityd's `check_naming` positively accepts exactly these two key IDs. Nevertheless vaultd has erased their encrypted objects and removed the bundle. This is a fault/data-loss reproduction, not an assertion that an unprivileged user can forge registry replies.

Service IPC §3/R14 and ADR 0006 §10 require reconciliation after **any** failure, timeout or restart, and erasure only on positive status evidence that held keys were superseded or can no longer be named. The positive status/unknown handling in `reconcile_recovery` is correct; this branch bypasses it.

Required: reconcile all recovery registration/naming outcomes through recovery_status. Preserve objects and the bundle whenever status is unavailable or says the held keys remain usable/nameable, irrespective of the original error's classification. Add this definitive-error case beside the existing storage-unavailable controls.

Reproducer: `defect_definitive_naming_error_does_not_erase_still_nameable_keys_without_status`. Debug and release: FAIL; the positive `check_naming == name` control passes, then the held persistent key's file is missing.

## Scope and contract assessment

| Check | Result | Evidence and limits |
|---|---|---|
| Exact range and docs-only follow-up | PASS | Three implementation commits, 35 changed files; subsequent d2c29ad changes only docs/. `git diff --exit-code` and `git status --porcelain` remained clean at the candidate after all tests. |
| Safe Rust and dependency boundaries | PASS | identityd/vaultd library roots forbid unsafe; package/workspace lints cover binary targets. No new local unsafe code. `git diff 20c849b..bb3f1ba -- core/native/os_adapter/src/lib.rs Cargo.lock`: only safe rustix memlock query/soft-limit raise, error mapping and its test; Cargo.lock adds only existing dependency edges, no crate or version. |
| Probe boundaries | PASS | Both vault binaries explicitly describe qualification-only scope. Seed/passphrase input in the key-object oracle probe is synthetic, not a deployed IPC route. No packaging/deployment addition in the range. |
| Recorded sequencing | PASS, bounded | registry/contract admission M2-005/006, deletion/replace_identity and ledger/crash recovery M2-006, transport/client/binaries/confinement M2-007 match the task package dependencies. This accepts staged implementation, not completion of A04/A05/A12 or waiver of those deferred operations. Missing replacement codes and deletion routes must be specified and reviewed in the later package. |
| Roles and user separation | PASS | caller.rs maps fixed validated accounts and regular UID + IPC GID only from peer credentials; root maps admin. Service methods enforce operation roles and owner binding. Own matrix, cross-user approval and key-substitution controls pass; actual new socket transport remains NOT_RUN until M2-007. |
| Layer-6 order and codes | PASS for historical-object checks | actor/key binding; invalidation, epoch, retirement; class; certificate window; then user scope/state. scope_of prioritizes key validity before user state. Recovery commit scope adds type/session binding. The current signing-time window is separately FAIL, R02. |
| Scenario differential | PASS within its projection | Own direct run: 12 key_authority scenarios, 318 comparisons, 0 disagreements, two tests passed. It verifies signed identity objects and commit layers 1–6, state records, modeled naming/status and retired/min-epoch observations. Admission/IPC errors for commits project to layer-6 success; registry-only grant records and append receipts are not compared. No claim of real registry writes/anchors or full admission. Scenario count 12, >=300 comparisons, no disagreements and expected-code assertions prevent a zero-comparison pass. |
| Registration and M2-003 obligation | PASS | Certified subject keys and enrolled keys call public_key_is_valid; certificates are verified with the registered identity key. Self-enrollment strictly extracts a candidate and re-verifies the envelope under it. Own bad-point, malformed-object and wrong-proof controls pass. Duplicate UID, windows, same-class supersedes, deleted-user retry and role checks are covered. |
| Restrictive invalidation and journal failures | FAIL overall | In-memory denial precedes journal access; append/read failure holds denial and returns restriction_not_durable as specified. Refresh/rebuild fails closed on unreadable or unappliable records. Own unavailable/recovery/corrupt-rebuild controls pass. Stronger pending compromise can be lost on a weaker retry: R01. Real ledger/restart/crash persistence is NOT_RUN, M2-006. |
| Key-object format/AAD | PASS | Closed deterministic map, fixed lengths/constants, all metadata in AAD, in-place AEAD in secret regions, recomputed public-key-derived ID. Independent 38-case oracle and own ciphertext/substitution controls pass. |
| KDF and workspace | PASS | Bounds and configured budget checked before derivation/allocation; exact-block workspace, clear after result and on drop; error clears output. Library regression control confirms clearing is necessary. Default 256 MiB/3/1 and locked budget 512 MiB; protection reserves 256 MiB headroom. |
| File store | PASS within protected-directory assumptions | Owner/mode/type/symlink/inode checks, bounded reads, temp/fsync/rename/directory-fsync; key metadata bound to path. Single vault writer/protected directory are prerequisites. Erasure's SSD/journal/COW limitation is stated. New full-system storage/crash testing remains NOT_RUN. |
| Proposal checks | FAIL overall | PASS role, schema/intrinsic, actor, key/owner/class, head/merge_base and epoch binding; canonical vault-built Sig_structure and digest equality; proposal expiry by either clock; immediate replay, attempt delays and fresh-passphrase decryption. R02–R04 expose signing-time authority/window and interleaved-ID reuse gaps. Stale current heads/epochs are authoritative admission checks in M2-005, not proved by these vault bindings. |
| Rotation/invalidation/recovery | FAIL overall | Correct normal-path ordering: encrypted objects + pending bundle durable before registration; rotation narrows old keys first; restored invalidated objects grant nothing. Own unknown-status, abort, held-key/restart and ceremony-scope controls pass. R05 violates all-error reconciliation. Named-key restoration, other-named cleanup and deletion integration need real registry/anchor tests in M2-006. |
| Process protection | PASS for implemented protection/probe | Hardening first, dumpable verification, sufficient memlock budget/headroom, then mlockall; errors refuse the protected probe. Own positive path verifies kernel flags and real release KDF; qualification tests the low-limit refusal. Service startup wiring and systemd/terminal/IPC protection remain NOT_RUN, M2-007. |
| Secret custody | PASS within approved limits | Detailed source/feature verification below. No raw-seed export or unrestricted signing operation in the service. No ordinary seed/KEK/passphrase storage in domain service paths; library copies are explicitly qualified. |

All 35 changed files were accounted for: identityd modules/tests; safe os_adapter delta; vault clock/KDF/key object/store/records/links/service/protection and both probes/tests; Cargo edges; workflow; dispatcher/inventory; independent vault oracle; defect harness and protection script. Supporting unchanged contract-format schema/intrinsic/envelope/test helpers and secure-memory APIs were followed where needed.

## Author decisions 1–8

1. **PASS as an in-process interface extension:** identity_state must convey registry records affecting authority for replay. Future wire pagination must still enforce IPC §2's byte bound and at-most-one signed object per message; a fixture page of 16 entries is not itself proof of compliant wire framing.
2. **PASS:** duplicate enrollment maps to duplicate_identifier; invalid certified point maps to key_not_certified, using approved vocabulary.
3. **PASS:** certificate windows and same-user/same-class predecessor checks narrow authority consistently with ADR 0005.
4. **PASS for design intent; FAIL for R01's error sequence:** accepting an already signed invalidation in quarantine follows IPC's every-state-except-deleted rule. The vault still cannot generate one in quarantine because ADR 0006 §4 denies every key. Escalating to compromised is consistent with its stronger historical effect, despite the oracle retaining the first reason. Both differences are outside the published vectors; the normal-path tests cover them, but the pending escalation is not preserved.
5. **PASS:** deletion preempts even identical registration retries; no success claim for a deleted identity.
6. **PASS:** exposing ceremony epoch/named keys through the authenticated identity answer supports the approved recovery flow.
7. **PASS:** corrupt/out-of-bounds storage, over-budget/protection and AEAD-authentication error distinctions are bounded and reveal no plaintext.
8. **PASS for stated staging:** system UTC object times are an explicit M2-004 limit; trusted-time integration is M2-006/007. This does not excuse R02's missing current-window check.

No owner decision is inferred from these assessments.

## Secret-custody source check

**PASS.** Sources were copied read-only from the built image's `/usr/share/cargo/registry` into `/workspace/m2-004-evidence/`; the qualification also checked the full pinned per-file source manifest. Inspect with `rg -n 'impl.*Drop|zeroize' <copied crate>/src`. Feature evidence: `cargo tree --frozen -p cham3_vaultd -e features -i poly1305` and the corresponding `-i chacha20`.

| Material | Verified ownership/cleanup |
|---|---|
| Service seeds | getrandom writes directly to secret_region_t; AEAD decrypts in place there; drop clears guarded/locked memory. |
| KEKs | derive_kek writes directly into a new secret_region_t; no generic Debug/Clone/serialization. |
| Passphrases | Service APIs take a secret_region_t reference. Its caller owns clearing/drop. Actual locked socket receipt and terminal custody are NOT_RUN, M2-007. |
| Argon2 workspace | Owned Vec<Block>, fixed allocation; explicit volatile block clear on every derivation result and drop. Whole-process locking protects it in deployed vault use; the unprotected unit fixture is not a protected daemon. argon2-0.5.3/lib.rs:323 clears initial hash, :503–504 clears final blocks, but does not clear caller workspace. |
| dalek copies | ed25519-dalek-2.1.1/signing.rs:652–654 clears SigningKey.secret_key; hazmat.rs:45–48 clears ExpandedSecretKey scalar/hash prefix. zeroize feature is enabled. |
| AEAD key | chacha20poly1305-0.10.1/lib.rs:292–297 unconditionally clears its key on Drop. |
| ChaCha state | chacha20-0.9.1/lib.rs:309–311 clears state under enabled zeroize feature; XChaCha wraps that core. |
| Poly1305 state | zeroize feature is absent in the actual tree. poly1305-0.8.0/backend/soft.rs:235–241 gates r/h/pad cleanup on it, so retained MAC state is not erased. chacha20poly1305/src/cipher.rs:41 does clear the initial local mac_key buffer; that does not erase the Poly1305 object's copied state. The handoff's limitation is accurate. |
| Other temporaries/death | Library stack copies and abort/death can bypass erasure. Approved coverage is mlockall, non-dumpable and core limit zero, not a claim that every RAM copy is cleared. Kernel/privileged compromise and hibernation are outside this guarantee. |

## Executed commands and results

All shell commands below ran as outer UID 1000. Candidate cwd is /workspace/m2-004-review unless stated otherwise. Logs were redirected outside the clone to /workspace/m2-004-evidence. Exact container commands:

```sh
docker build -f build/m2/dev_container.dockerfile -t cham3-m2-dev build/m2

docker run --rm --user "$(id -u):$(id -g)" \
  -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -v /workspace/m2-004-review:/work -w /work cham3-m2-dev \
  bash build/scripts/qualify_m2_rust.sh

docker run --rm --user "$(id -u):$(id -g)" \
  --ulimit memlock=1073741824:1073741824 -e HOME=/tmp/cham3_home \
  -v /workspace/m2-004-review:/work -w /work cham3-m2-dev \
  sh tests/m2/protection/check_vault_protection.sh out/m2/protection

docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -v /workspace/m2-004-review:/work -w /work cham3-m2-dev \
  bash build/scripts/run_static_checks.sh

docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -v /workspace/m2-004-review:/work -w /work cham3-m2-dev \
  sh tests/run_tests.sh static
```

| Execution | Status / exit | Actual result |
|---|---|---|
| Pinned image build | PASS / 0 | Exact input image and packages; later full installed/deb locks match. |
| Clean amd64 qualification | PASS / 0 | source_identity: candidate hash, dirty_tracked 0, Cargo.lock SHA-256 da1a17c2bf6916e145413574a7f43782048a228962789f387b9d217c78f2e28f. rustfmt/clippy/naming, debug/release, backend/profile and valgrind checks all passed. |
| Unit | PASS | 114 debug + 114 release, inventory-exact; no required skips. Provider probe 7/7; independent provider checks pass, with the existing explicit NOT_CHECKED for libargon2's RFC secret/AD interface (the Rust KAT and independent ordinary-input match run). ECR 19 valid/96 invalid/18 admission, 22 journal scenarios/346 steps. Format differential 5950 cases/0 mismatches. Vault-object oracle CHECKS 38, all PASS, plus its own AEAD control. |
| Harness | PASS | 133 Python tests, including one test that independently compiles/runs and detects all 14 seeded identity/vault defects and the clean controls. Defect detection requires a named test failure, not a compiler failure. The small-memlock release protection refusal is included. |
| Protection positive | PASS / 0 | required_memlock_bytes 805306368, derivation_ms 515, dumpable 0, core_limit 0, vm_locked_kib 3152, unlocked_mappings 0, special_mappings [vdso], dontdump_mappings 4, protection_verified. |
| Static checks | PASS / 0 | shellcheck, py_compile, yamllint, scan_secrets. |
| Static tests | PASS / 0 | 82 tests, 65.779 seconds. |
| Own final adversarial debug | FAIL / 101 | 17 tests: 11 controls PASS, six assertions FAIL (five reportable findings and one stronger-than-spec authentication expectation discussed below); 0 ignored/filtered, 59.54 s. |
| Own final adversarial release | FAIL / 101 | Same 17 tests, same 11/6 result, 0 ignored/filtered, 3.55 s. Release cargo tests still use the unwind test harness; they do not prove deployment abort behavior. The separate release protection probe does run the deployed profile. |
| Own direct scenario run | PASS / 0 | 12 scenarios/318 comparisons/0 disagreements; 2 tests. |
| Security plugin workflow | BLOCKED | See exact preflight/resource/tool availability below. No plugin scan or plugin findings are claimed. |
| Miri/sanitizers/coverage-guided fuzzing | NOT_RUN | Qualification explicitly records toolchain/package unavailability. Bounded parser mutation/property tests and valgrind are narrower evidence. |
| New real IPC/flooding, systemd/client custody, registry anchor/ledger/crashes, replacement/deletion, whole-system enforcement | NOT_RUN | M2-005–007 implementations absent at this candidate. These remain mandatory later milestones. |

Qualification directories: `out/m2/qualification/amd64_20261013T035220Z_1/`, `out/m2/runs/unit_20261013T035320Z_1768/`, `out/m2/runs/harness_20261013T035623Z_3777/`; positive report `out/m2/protection/vault_protection_report.txt`.

The adversarial fixture uses real Argon2id at the approved minimum 64 MiB/1/1, XChaCha20-Poly1305 and Ed25519. Its journal/registry and time scheduling are labeled in-process fixtures. Eleven controls cover caller matrices/peer range edges, cross-user approval and key substitution, forged digest/wrong passphrase/immediate replay, each-clock expiry equality, malformed/head/epoch binding, key-object tamper/substitution with restored positive control, recovery unknown status/abort/restart/ceremony scope, malformed enrollment and certified points, and journal read faults/corrupt rebuild.

### Additional observation I01 — recovery completion is not fresh signing

The sixth failed experimental assertion, `defect_recovery_retry_authenticates_passphrase_before_naming`, observes that a pending recovery resumes naming with a wrong passphrase. **PASS against R14's reconciliation rule; FAIL against the stronger experimental authentication expectation.** No new key or signature is generated in this path: held objects and identity-signed certificates came from the original passphrase-authenticated operation. R14 requires completing such held bundles, and register_proposal also invokes complete_pending without a passphrase. This is not reported as a fresh-signing bypass or owner decision. The handoff's general statement that every retry needs the held keys' passphrase is overbroad; clarify the difference between enrollment's authentication check and recovery finalization.

## CI comparison and artifact limit

**PASS for run identity, job/step outcomes and downloaded text logs; BLOCKED for local ZIP inspection/per-file artifact comparison.**

Exact read calls (connected GitHub tools, repo_full_name Kayyo321/cham3):

- `github_fetch(url="https://api.github.com/repos/Kayyo321/cham3/actions/runs/<id>")` for 38341512299, 38341511647, 38341511661: every head_sha bb3f1ba, completed/success, push event.
- `github_fetch_workflow_run_jobs(run_id=<id>)` for each; all jobs and relevant steps success.
- `github_fetch_workflow_job_logs(job_id=<id>)` for 115113321971, 115113319766, 115113319993, 115113320022, 115113319928, 115113320140, 115113320184: all complete text logs independently retrieved.
- `github_fetch_workflow_run_artifacts(run_id=38341512299,name="m2_qualification_amd64")`: id 11787038783, 96660 bytes, not expired, advertised digest `sha256:ca8945506ae171139f8b994d8fcd7373fe05382f314b6d3c7d32f00ccc7d361e`, matching candidate.
- `github_download_workflow_artifact(artifact_id=11787038783,file_name="m2_qualification_amd64.zip")` returned a File Service reference. The executor could not materialize it (curl HTTP 403); the advertised digest is metadata, not a locally computed ZIP hash.
- Required CLI attempt: `gh run download 38341512299 -n m2_qualification_amd64 --repo Kayyo321/cham3 --dir /workspace/m2-004-evidence/ci-gh`: BLOCKED, api.github.com Forbidden. `gh run view 38341512299 --repo Kayyo321/cham3 --json headSha,status,conclusion,jobs,url` was blocked identically; connector fallback succeeded.

m2 job 115113321971: qualification PASS and positive protection step PASS. Its report matches the local positive flags/counts, with derivation_ms 431 instead of 515. The CI workflow uses --record; the independent local run did not. Neither run used privilege/capability additions for the protection path. The VM_LOCKED lo flag, rather than proportional Locked/Rss sizes, is the correct pinned probe criterion; special kernel mappings are explicitly excluded.

m1 static_and_desktop: static checks PASS, static 82, desktop 261, staging 16, native pin-identity 22 tests. privileged_capture: 77 tests PASS (CI-only disposable privileged suite). package_build: 8 PASS.

m0 static_checks: static checks PASS, static 82. image_build: package/image builds PASS, image 17, boot reaches serial login prompt, boot-result 18 tests PASS. package_build: 8 PASS.

The local requested unit/harness counts were independently reproduced. CI logs confirm suite success but do not expose the full uploaded per-test inventories; those remain BLOCKED for artifact inspection. This limitation must not be described as a completed ZIP comparison.

## Codex Security plugin

Requested exact skill: **codex-security:security-diff-scan**; package `skill://Plugin_1e648473be9c8191a91ac3947151af55/security-diff-scan`. **Version: unavailable** (no version in provided skill metadata/front matter, no installed plugin manifest). Result: **BLOCKED**, not a completed scan.

Executed:

1. `skills.read({"package":"c0/security-diff-scan"})`: PASS, read its workflow.
2. `skills.read({"package":"c0/security-diff-scan","resource":"skill://Plugin_1e648473be9c8191a91ac3947151af55/references/config-preflight.md"})`: BLOCKED, "failed to read skill resource". Attempts using the supplied relative traversal and skills/references and skill-local reference forms failed as well.
3. `skills.list({"authority":{"kind":"executor"}})`: empty skills list.
4. Tool inventory search for codex_security/start/scan/preflight capabilities: no Codex Security MCP methods exposed. Terminal fallback search `rg --files --hidden /opt/codex /workspace /home/agent -g config-preflight.md -g launch_codex_security_mcp -g generate_in_scope_files.py -g finalize_scan_contract.py`: no matches. `command -v launch_codex_security_mcp`: absent.

The skill explicitly requires capability preflight before review/goal creation and provides terminal launch/generation/finalization scripts as fallback. Neither the preflight resource nor callable backend/scripts is available here. No scan ID/goal was created, no remediation was applied, and no independent manual finding is mislabeled as a plugin finding. The plugin remains required for final M2-008 review.

## Evidence integrity

Unpublished local evidence is retained in /workspace/m2-004-evidence and the candidate's ignored out/. Only this review record is published. Key SHA-256 digests:

| Local artifact | SHA-256 |
|---|---|
| docker-build.log | 2e3dcad3408186911b87663a16afb011f091340066ff2491bc83bc0fc94684e2 |
| qualification.log | fc99195cc39048dba14ac17e184e91ef24c4c06e2c57e61fe276441a811b5409 |
| protection-positive.log | d8662a334c2920336ed18bb24378d7f50ecce684827c6be3ac8109b9c27dff41 |
| static-checks.log | 4de7404dff863ec97ff5b62d03e3459cd696c6f71478443d6d00749f021f0bfb |
| static-tests.log | a9c921d71a30616bd11d5a39e0cd7b669f32dd8861b731b2999d8a6462063226 |
| adversarial-final-debug.log | cfee13e16ad8ffc9e2203e6bedf09cc7e1469ae1ad0963bd0062e964ffc03712 |
| adversarial-final-release.log | 0b8e1a7fe35a2bbfc2c5ba0ca52e778be6c4d7555d8a7e7ccf5947405406c97e |
| ci-m2-job.log | c26c7b551c339ccd259ca3814e1293acc6f04b752aaaa80d19ae76f2ea5c56e6 |
| unit cargo_test_debug.json | 946496196251cb7c5e8f889323a5314f29d20f57bb52a8fbb0cc762373c2398b |
| unit cargo_test_release.json | a3bc72982862d6e9f057ba15fe85e11de90f1adb8447255b42c7b2c4f9150978 |
| format_differential.json | e29922e3a435d43ea1ddb8b85ba2e08d7441a3bdbb812caf2de62ba433bd6d50 |
| harness python_harness.json | 32aacd1fd8389d108c99f4eaf99785fcb4a90543ddac8842dadc812bc9dd32eb |
| final adversarial review.rs | 922dd57b9ba92ce7b79517c6a3f890a1129626dc61766af6b814ee13df15bea4 |
| adversarial Cargo.toml | 82224a3c0e00b6cc3ac9c6b4ceaebc810c7d1912ff32366ecfdc8941ba961004 |

The main publication needs no implementation merge. Integrator/coordinator should return R01–R05 for implementation and exact-candidate re-review; preserve later package obligations and the M2-008 plugin requirement.

## Reproducible independent test appendix

The full external test source and manifest follow. They import only candidate test builders/fixtures and public service APIs; /work is mounted read-only for these tests. They do not patch implementation files.

Prepare /workspace/m2-004-evidence/adversarial/.cargo/config.toml by copying the candidate .cargo/config.toml, place the following manifest and review.rs under adversarial/core/reviewer/, and copy the candidate vectors.json to adversarial/tests/m2/vectors/ecr_v1/vectors.json (the imported test helper uses CARGO_MANIFEST_DIR/../../tests). Initial `cargo test --offline` generated the external package's lockfile only.

Exact final commands:

```sh
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -e CARGO_TARGET_DIR=/evidence/adversarial/target \
  -v /workspace/m2-004-review:/work:ro -v /workspace/m2-004-evidence:/evidence \
  -w /evidence/adversarial cham3-m2-dev \
  cargo test --manifest-path core/reviewer/Cargo.toml --locked --offline \
  -- --nocapture --test-threads=1

docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -e CARGO_TARGET_DIR=/evidence/adversarial/target \
  -v /workspace/m2-004-review:/work:ro -v /workspace/m2-004-evidence:/evidence \
  -w /evidence/adversarial cham3-m2-dev \
  cargo test --manifest-path core/reviewer/Cargo.toml --release --locked --offline \
  -- --nocapture --test-threads=1
```

Initial 16-test debug/release runs reproduced the first five experimental failures; a targeted added definitive-naming-error run reproduced R05 (exit 101). The final 17-test runs above cover the final source. The direct differential/feature check used the same image, user and read-only candidate mount, with:

```sh
CARGO_TARGET_DIR=/evidence/differential-target cargo test --frozen \
  -p cham3_identityd --test journal_scenarios -- --nocapture
cargo tree --frozen -p cham3_vaultd -e features -i poly1305
cargo tree --frozen -p cham3_vaultd -e features -i chacha20
```

The failure assertions are normative tests, except explicitly qualified I01. Their failing process exit is expected evidence of the candidate behavior, not a harness/environment failure.

### External Cargo.toml

```toml
[package]
name = "m2_004_independent_review_tests"
version = "0.1.0"
edition = "2024"

[dependencies]
cham3_vaultd = { path = "/work/core/vaultd" }
cham3_identityd = { path = "/work/core/identityd" }
cham3_contract_format = { path = "/work/core/contract_format" }
cham3_secure_memory = { path = "/work/core/native/secure_memory" }
cham3_os_adapter = { path = "/work/core/native/os_adapter" }
ed25519-dalek = { version = "=2.1.1", default-features = false, features = ["std", "zeroize"] }

[lib]
path = "review.rs"

[profile.release]
overflow-checks = true
codegen-units = 1
incremental = false
```

### External review.rs

```rust
#![forbid(unsafe_code)]
#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]
#[path = "/work/core/vaultd/tests/fixture/mod.rs"]
mod fixture;
#[path = "/work/core/identityd/tests/builders/mod.rs"]
mod builders;
use fixture::*;
use std::cell::Cell;
use std::rc::Rc;
use cham3_contract_format::{object_kind_t, key_resolver_t, verify_object};
use cham3_contract_format::schema::{authority_class_t, invalidation_reason_t, commit_payload_t};
use cham3_contract_format::hash::digest_t;
use cham3_identityd::caller::{caller_role_t, service_name_t, role_map_t};
use cham3_identityd::index::{ceremony_mode_t, record_role_t, user_record_t, key_scope_t};
use cham3_identityd::journal::memory_journal_t;
use cham3_identityd::service::identity_service_t;
use cham3_identityd::outcome::service_error_t;
use cham3_vaultd::service::{proposal_request_t, created_keys_t, vault_service_t};
use cham3_vaultd::links::in_process_identity_t;
use cham3_vaultd::clock::vault_clock_t;
const Alice: caller_role_t = caller_role_t::trusted_client { uid: 1001 };
const Bob: caller_role_t = caller_role_t::trusted_client { uid: 1002 };
const Contractd: caller_role_t = caller_role_t::service(service_name_t::contractd);
const Vaultd: caller_role_t = caller_role_t::service(service_name_t::vaultd);
const Pass: &str = "review synthetic secret";
const At: &str = "2026-10-13T08:00:30Z";
fn code<T>(r: Result<T, service_error_t>) -> String { r.err().map(|e| e.to_string()).unwrap_or_else(|| "ACCEPTED".into()) }
fn setup(label: &str) -> (std::path::PathBuf, identity_t, vault_t, created_keys_t) {
    let root = temp_root(label); let identity = identity_service(); let mut v = vault(&root, &identity);
    let keys = v.create_user_keys(Alice, &passphrase(Pass)).unwrap(); (root,identity,v,keys)
}
fn payload(k: &created_keys_t) -> commit_payload_t {
    commit_payload("commit_genesis",1001,&k.user_id,&k.persistent_key_id,authority_class_t::persistent,At)
}
fn request(label: &str,p: &commit_payload_t) -> proposal_request_t {
    proposal_request_t { proposal_id: proposal_id(label),owner_uid:1001,kind:object_kind_t::commit,key_id:p.key_id,
        expected_head:trunk_parent(p),policy_epoch:p.policy.epoch,expires_in_seconds:120,payload:encoded(p) }
}
struct resolver_t(identity_t);
impl key_resolver_t for resolver_t { fn resolve(&self,kid:&digest_t)->Option<[u8;32]> {
    self.0.borrow_mut().resolve_key(Contractd,kid).ok().map(|k|k.public_key)
}}
fn open(i:&identity_t,id:[u8;16],epoch:u64) {
    push_record(i,1001,user_record_t::containment);
    push_record(i,1001,user_record_t::recovery_opened { recovery_id:id,key_epoch:epoch,mode:ceremony_mode_t::c1,role:record_role_t::trusted_client });
}
#[test] fn controls_role_matrix_and_cross_user_approval() {
    let (_,i,mut v,k)=setup("review_roles"); let p=payload(&k);
    for c in [Alice,Bob,caller_role_t::admin,caller_role_t::fixture_consumer,
        Vaultd,caller_role_t::service(service_name_t::identityd),caller_role_t::service(service_name_t::registryd),caller_role_t::service(service_name_t::policyd)] {
        assert_eq!(code(v.register_proposal(c,request("role",&p))),"ipc/role_denied");
    }
    let r=v.register_proposal(Contractd,request("own",&p)).unwrap();
    for c in [caller_role_t::admin,caller_role_t::fixture_consumer,Contractd,Vaultd] {
        assert_eq!(code(v.approve_and_sign(c,&r.proposal_id,&r.approval_digest,&passphrase(Pass))),"ipc/role_denied");
    }
    assert_eq!(code(v.approve_and_sign(Bob,&r.proposal_id,&r.approval_digest,&passphrase(Pass))),"ipc/owner_mismatch");
    let signature=v.approve_and_sign(Alice,&r.proposal_id,&r.approval_digest,&passphrase(Pass)).unwrap();
    verify_object(&signature,object_kind_t::commit,&resolver_t(i)).unwrap();
}
#[test] fn controls_peer_credential_edges() {
    let m=role_map_t::new([(service_name_t::identityd,991),(service_name_t::vaultd,992),(service_name_t::contractd,993),(service_name_t::policyd,994),(service_name_t::registryd,995)],990,Some(989),1000,60000).unwrap();
    for uid in [999,1000,1001,60000,60001] { for gid in [0,990,1001] {
        let role=m.role_of(&cham3_os_adapter::peer_credentials_t{pid:123,uid,gid});
        assert_eq!(role.is_some(),(1000..=60000).contains(&uid)&&gid==990);
    }}
}
#[test] fn controls_forged_approval_and_immediate_replay() {
    let (_,_,mut v,k)=setup("review_approval"); let r=v.register_proposal(Contractd,request("r",&payload(&k))).unwrap();
    assert_eq!(code(v.approve_and_sign(Alice,&r.proposal_id,&[0;32],&passphrase(Pass))),"ipc/approval_mismatch");
    assert_eq!(code(v.approve_and_sign(Alice,&r.proposal_id,&r.approval_digest,&passphrase("wrong"))),"ipc/authentication_failed");
    v.approve_and_sign(Alice,&r.proposal_id,&r.approval_digest,&passphrase(Pass)).unwrap();
    assert_eq!(code(v.approve_and_sign(Alice,&r.proposal_id,&r.approval_digest,&passphrase(Pass))),"ipc/proposal_used");
}
#[test] fn controls_each_clock_expires_at_equality() {
    for mono in [true,false] {
        let (_,_,mut v,k)=setup(if mono {"review_mono"}else{"review_wall"});
        let r=v.register_proposal(Contractd,request("r",&payload(&k))).unwrap();
        if mono { v.clock_mut().monotonic+=120; v.clock_mut().utc-=3600; } else { v.clock_mut().utc+=120; }
        assert_eq!(code(v.approve_and_sign(Alice,&r.proposal_id,&r.approval_digest,&passphrase(Pass))),"ipc/proposal_expired");
    }
}
#[test] fn controls_head_epoch_binding_and_malformed_payloads() {
    let (_,_,mut v,k)=setup("review_binding");let p=payload(&k);
    let mut r=request("head",&p);r.expected_head=Some([4;32]);assert_eq!(code(v.register_proposal(Contractd,r)),"ipc/malformed_request");
    let mut r=request("epoch",&p);r.policy_epoch+=1;assert_eq!(code(v.register_proposal(Contractd,r)),"ipc/malformed_request");
    for bytes in [vec![],vec![0xff],vec![0xbf,0xff],vec![0xa0],vec![0xa1,0x61,b'x',0]] {
        let mut r=request("malformed",&p);r.payload=bytes;assert_ne!(code(v.register_proposal(Contractd,r)),"ACCEPTED");
    }
}
#[test] fn controls_substituted_and_tampered_key_objects() {
    let (_,_,mut v,k)=setup("review_tamper");let p=payload(&k);
    let r=v.register_proposal(Contractd,request("tamper",&p)).unwrap();
    let store=v.store();let mut o=store.read_key(1001,&k.persistent_key_id).unwrap().unwrap();
    let path=store.root().join("1001").join(format!("{}.key",fixture::support::hex_encode(&k.persistent_key_id)));
    let original=o.encode();o.ciphertext[0]^=1;std::fs::write(&path,o.encode()).unwrap();
    assert_eq!(code(v.approve_and_sign(Alice,&r.proposal_id,&r.approval_digest,&passphrase(Pass))),"ipc/authentication_failed");
    let other=v.store().read_key(1001,&k.temporary_key_id).unwrap().unwrap();
    std::fs::write(&path,other.encode()).unwrap();
    assert_eq!(code(v.approve_and_sign(Alice,&r.proposal_id,&r.approval_digest,&passphrase(Pass))),"ipc/storage_unavailable");
    std::fs::write(&path,original).unwrap();
    v.approve_and_sign(Alice,&r.proposal_id,&r.approval_digest,&passphrase(Pass)).unwrap();
}
#[test] fn controls_cross_user_key_substitution() {
    let (_,_,mut v,k)=setup("review_foreign");let bob=v.create_user_keys(Bob,&passphrase("bob secret")).unwrap();
    let mut p=payload(&k);p.key_id=bob.persistent_key_id;
    assert_eq!(code(v.register_proposal(Contractd,request("foreign",&p))),"key_authority/key_not_certified");
    let mut r=request("foreign_uid",&payload(&k));r.owner_uid=1002;
    assert_eq!(code(v.register_proposal(Contractd,r)),"ipc/owner_mismatch");
}
#[test] fn controls_recovery_unknown_status_keeps_objects_and_aborted_ceremony_cleans() {
    let (_,i,mut v,_)=setup("review_reconcile");let id=[7;16];open(&i,id,2);
    v.registry_mut().naming_unavailable=true;v.registry_mut().status_unavailable=true;
    assert_eq!(code(v.recover_keys(Alice,&id,&passphrase(Pass))),"ipc/storage_unavailable");
    let held=v.store().list_keys(1001).unwrap();assert_eq!(held.len(),5);
    assert_eq!(code(v.recover_keys(Alice,&id,&passphrase(Pass))),"ipc/storage_unavailable");
    assert_eq!(held,v.store().list_keys(1001).unwrap());
    push_record(&i,1001,user_record_t::containment);
    v.registry_mut().naming_unavailable=false;v.registry_mut().status_unavailable=false;
    assert_eq!(code(v.recover_keys(Alice,&id,&passphrase(Pass))),"admission/recovery_rule");
    assert_eq!(v.store().list_keys(1001).unwrap().len(),3);
}
#[test] fn controls_ceremony_scope_and_named_objects_survive_restart() {
    let (root,i,mut v,_)=setup("review_scope");let id=[8;16];open(&i,id,2);
    v.registry_mut().anchor_fault=true;assert_eq!(code(v.recover_keys(Alice,&id,&passphrase(Pass))),"ipc/storage_unavailable");
    let held=v.store().list_keys(1001).unwrap();assert_eq!(held.len(),5);
    let pending=v.registry_mut().pending;
    drop(v);let mut reg=registry(&i);reg.pending=pending;
    let mut v=vault_with(&root,&i,reg);let named=v.recover_keys(Alice,&id,&passphrase(Pass)).unwrap();
    assert!(v.store().read_key(1001,&named.0).unwrap().is_some());
    assert!(v.store().read_key(1001,&named.1).unwrap().is_some());
    let user=i.borrow_mut().user_identity(Contractd,1001).unwrap();
    let k=created_keys_t { user_id:user.user_id,persistent_key_id:named.0,temporary_key_id:named.1 };
    assert_eq!(code(v.register_proposal(Contractd,request("nonceremony",&payload(&k)))),"admission/recovery_rule");
    push_record(&i,1001,user_record_t::recovery_closed{recovery_id:id});
    let r=v.register_proposal(Contractd,request("closed",&payload(&k))).unwrap();
    v.approve_and_sign(Alice,&r.proposal_id,&r.approval_digest,&passphrase(Pass)).unwrap();
}
fn enrolled_identity() -> (identity_service_t<memory_journal_t>, builders::test_user_t) {
    let u=builders::test_user(1001);let mut i=identity_service_t::start(memory_journal_t::new()).unwrap();
    i.register_enrollment(Vaultd,1001,&u.enrollment,&u.persistent_certificate,&u.temporary_certificate).unwrap();(i,u)
}
#[test] fn controls_malformed_identity_proof_and_bad_certified_points() {
    let (mut i,u)=enrolled_identity();let new=builders::test_key("review bad cert");
    for bytes in [vec![],vec![0xff],vec![0xd2,0x84,0x40,0xa0,0x40,0x40]] {
        assert_ne!(code(i.register_certificate(Vaultd,1001,&bytes)),"ACCEPTED");
    }
    for point in builders::BadPoints {
        let mut cert=builders::certificate_payload(&u.identity,1001,&new,authority_class_t::persistent,1);
        cert.subject_public_key=point;cert.subject_key_id=cham3_contract_format::hash::key_id(&point);
        assert_eq!(code(i.register_certificate(Vaultd,1001,&builders::sign_certificate(&u.identity,&cert))),"key_authority/key_not_certified");
    }
    let other=builders::test_user(1002);let mut forged=builders::enrollment_payload(&other.identity,1002);
    forged.public_key=u.identity.public_key;forged.user_id=u.identity.key_id;
    let bytes=builders::sign_object(object_kind_t::identity_enrollment,&other.identity,&forged.to_value());
    assert_ne!(code(i.register_enrollment(Vaultd,1002,&bytes,&other.persistent_certificate,&other.temporary_certificate)),"ACCEPTED");
}
#[test] fn controls_journal_fault_and_corrupt_rebuild() {
    let (mut i,u)=enrolled_identity();i.journal_mut().faults.unavailable=true;
    assert_eq!(code(i.resolve_key(Contractd,&u.persistent.key_id)),"ipc/storage_unavailable");
    i.journal_mut().faults.unavailable=false;assert_eq!(i.resolve_key(Contractd,&u.persistent.key_id).unwrap().scope,key_scope_t::normal);
    i.journal_mut().push_user_record(1001,user_record_t::recovery_closed{recovery_id:[9;16]}).unwrap();
    assert_eq!(code(i.resolve_key(Contractd,&u.persistent.key_id)),"ipc/storage_unavailable");
    let i=identity_service_t::start(i.journal().clone()).unwrap();
    assert!(i.directory().user(1001).is_none());
}
#[test] fn defect_pending_compromise_is_not_cleared_by_old_rotated_retry() {
    let (mut i,u)=enrolled_identity();
    let rotated=builders::invalidation(&u.identity,&u.persistent.key_id,invalidation_reason_t::rotated,"rotated");
    i.register_invalidation(Vaultd,1001,&rotated).unwrap();
    let compromised=builders::invalidation(&u.identity,&u.persistent.key_id,invalidation_reason_t::compromised,"compromised");
    i.journal_mut().faults.fail_appends=true;
    assert_eq!(code(i.register_invalidation(Vaultd,1001,&compromised)),"ipc/restriction_not_durable");
    assert_eq!(i.resolve_key(Contractd,&u.persistent.key_id).unwrap().invalidated,Some(invalidation_reason_t::compromised));
    i.journal_mut().faults.fail_appends=false;i.register_invalidation(Vaultd,1001,&rotated).unwrap();
    assert_eq!(i.resolve_key(Contractd,&u.persistent.key_id).unwrap().invalidated,Some(invalidation_reason_t::compromised),
        "an old durable retry must not discard the stronger pending restriction");
}
#[test] fn defect_used_proposal_identifier_cannot_be_re_registered_after_gc() {
    let (_,_,mut v,k)=setup("review_gc");let p=payload(&k);
    let r=v.register_proposal(Contractd,request("original",&p)).unwrap();
    let first=v.approve_and_sign(Alice,&r.proposal_id,&r.approval_digest,&passphrase(Pass)).unwrap();
    v.register_proposal(Contractd,request("trigger_gc",&p)).unwrap();
    let replay=v.register_proposal(Contractd,request("original",&p));
    if let Ok(r)=&replay {
        let second=v.approve_and_sign(Alice,&r.proposal_id,&r.approval_digest,&passphrase(Pass)).unwrap();
        assert_eq!(first,second,"positive reproduction: same bytes signed again");
    }
    assert_eq!(code(replay),"ipc/duplicate_identifier","used proposal IDs remain single use");
}
#[test] fn defect_certificate_expiry_during_live_proposal_prevents_signing() {
    let (_,i,mut v,k)=setup("review_window");let before=StartUtc+7*86400-5;
    v.clock_mut().utc=before;v.clock_mut().monotonic+=7*86400-5;
    let p=commit_payload("commit_temporary_session",1001,&k.user_id,&k.temporary_key_id,authority_class_t::temporary,"2026-10-20T07:59:55Z");
    let r=v.register_proposal(Contractd,request("near_window_end",&p)).unwrap();
    v.clock_mut().utc+=6;v.clock_mut().monotonic+=6;
    let result=v.approve_and_sign(Alice,&r.proposal_id,&r.approval_digest,&passphrase(Pass));
    if let Ok(bytes)=&result { verify_object(bytes,object_kind_t::commit,&resolver_t(i)).unwrap(); }
    assert_eq!(code(result),"key_authority/certificate_window",
        "a still-live proposal cannot extend the signing key's certificate window");
}
#[test] fn defect_recovery_retry_authenticates_passphrase_before_naming() {
    let (_,i,mut v,_)=setup("review_retry_password");let id=[10;16];open(&i,id,2);
    v.registry_mut().naming_unavailable=true;v.registry_mut().status_unavailable=true;
    assert_eq!(code(v.recover_keys(Alice,&id,&passphrase(Pass))),"ipc/storage_unavailable");
    v.registry_mut().naming_unavailable=false;v.registry_mut().status_unavailable=false;
    assert_eq!(code(v.recover_keys(Alice,&id,&passphrase("wrong"))),"ipc/authentication_failed",
        "retry claiming passphrase authentication must actually check it");
}
struct changing_clock_t { identity:identity_t, armed:Rc<Cell<u32>> }
struct denied_registry_t;
impl cham3_vaultd::links::recovery_registry_t for denied_registry_t {
    fn recovery_status(&mut self,_:u32,_:&[u8;16])->Result<cham3_identityd::index::recovery_status_t,service_error_t> {
        Err(service_error_t::ipc(cham3_identityd::outcome::ipc_code_t::storage_unavailable))
    }
    fn name_recovery_keys(&mut self,_:u32,_:&cham3_identityd::index::naming_t)->Result<cham3_identityd::index::naming_effect_t,service_error_t> {
        Err(service_error_t::ipc(cham3_identityd::outcome::ipc_code_t::role_denied))
    }
}
#[test] fn defect_definitive_naming_error_does_not_erase_still_nameable_keys_without_status() {
    let root=temp_root("review_naming_error");let i=identity_service();
    let mut v=vault_service_t::start(config(&root),in_process_identity_t{service:i.clone()},denied_registry_t,
        test_clock_t{monotonic:1000,utc:StartUtc}).unwrap();
    v.create_user_keys(Alice,&passphrase(Pass)).unwrap();
    let id=[11;16];open(&i,id,2);
    assert_eq!(code(v.recover_keys(Alice,&id,&passphrase(Pass))),"ipc/role_denied");
    let named={
        let i=i.borrow();let user=i.directory().user(1001).unwrap();
        let key=|class| *user.keys().find(|(_,k)|k.key_epoch==2&&k.class==class).unwrap().0;
        let named=cham3_identityd::index::naming_t{ recovery_id:id,
            persistent_key_id:key(cham3_identityd::index::key_class_t::persistent),
            temporary_key_id:key(cham3_identityd::index::key_class_t::temporary)};
        assert_eq!(user.check_naming(1001,&named).unwrap(),cham3_identityd::index::naming_effect_t::name,
            "positive control: both certificates remain durable and can be named");
        named
    };
    assert!(v.store().read_key(1001,&named.persistent_key_id).unwrap().is_some(),
        "an error is not positive recovery_status evidence that held keys can never be named");
}
impl vault_clock_t for changing_clock_t {
    fn monotonic_seconds(&self)->u64 {
        let n=self.armed.get();
        if n>0 { self.armed.set(n-1); if n==1 {
            push_record(&self.identity,1001,user_record_t::deletion_pending);
        }} 1000
    }
    fn utc_seconds(&self)->i64 { StartUtc }
}
#[test] fn defect_rechecks_authority_after_unlock_before_signature() {
    let root=temp_root("review_sign_race");let i=identity_service();let armed=Rc::new(Cell::new(0));
    let mut v=vault_service_t::start(config(&root),in_process_identity_t{service:i.clone()},registry(&i),
        changing_clock_t{identity:i.clone(),armed:armed.clone()}).unwrap();
    let k=v.create_user_keys(Alice,&passphrase(Pass)).unwrap();
    let r=v.register_proposal(Contractd,request("race",&payload(&k))).unwrap();
    armed.set(2); // first expiry read precedes identity resolution; second follows KDF/decryption.
    let result=v.approve_and_sign(Alice,&r.proposal_id,&r.approval_digest,&passphrase(Pass));
    if let Ok(bytes)=&result { verify_object(bytes,object_kind_t::commit,&resolver_t(i.clone())).unwrap(); }
    assert_eq!(i.borrow_mut().resolve_key(Contractd,&k.persistent_key_id).unwrap().scope,
        key_scope_t::denied(cham3_identityd::outcome::authority_code_t::user_deleted));
    assert_eq!(code(result),"key_authority/user_deleted","deletion became visible before the actual signature");
}
```

### Final release result excerpt

```text
running 17 tests
test controls_ceremony_scope_and_named_objects_survive_restart ...      Running unittests review.rs (target/release/deps/m2_004_independent_review_tests-807dec036a47220c)
ok
test controls_cross_user_key_substitution ... ok
test controls_each_clock_expires_at_equality ... ok
test controls_forged_approval_and_immediate_replay ... ok
test controls_head_epoch_binding_and_malformed_payloads ... ok
test controls_journal_fault_and_corrupt_rebuild ... ok
test controls_malformed_identity_proof_and_bad_certified_points ... ok
test controls_peer_credential_edges ... ok
test controls_recovery_unknown_status_keeps_objects_and_aborted_ceremony_cleans ... ok
test controls_role_matrix_and_cross_user_approval ... ok
test controls_substituted_and_tampered_key_objects ... ok
test defect_certificate_expiry_during_live_proposal_prevents_signing ...
thread 'defect_certificate_expiry_during_live_proposal_prevents_signing' panicked at review.rs:199:5:
assertion `left == right` failed: a still-live proposal cannot extend the signing key's certificate window
  left: "ACCEPTED"
 right: "key_authority/certificate_window"
note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
FAILED
test defect_definitive_naming_error_does_not_erase_still_nameable_keys_without_status ...
thread 'defect_definitive_naming_error_does_not_erase_still_nameable_keys_without_status' panicked at review.rs:237:5:
an error is not positive recovery_status evidence that held keys can never be named
FAILED
test defect_pending_compromise_is_not_cleared_by_old_rotated_retry ...
thread 'defect_pending_compromise_is_not_cleared_by_old_rotated_retry' panicked at review.rs:176:5:
assertion `left == right` failed: an old durable retry must not discard the stronger pending restriction
  left: Some(rotated)
 right: Some(compromised)
FAILED
test defect_rechecks_authority_after_unlock_before_signature ...
thread 'defect_rechecks_authority_after_unlock_before_signature' panicked at review.rs:260:5:
assertion `left == right` failed: deletion became visible before the actual signature
  left: "ACCEPTED"
 right: "key_authority/user_deleted"
FAILED
test defect_recovery_retry_authenticates_passphrase_before_naming ...
thread 'defect_recovery_retry_authenticates_passphrase_before_naming' panicked at review.rs:207:5:
assertion `left == right` failed: retry claiming passphrase authentication must actually check it
  left: "ACCEPTED"
 right: "ipc/authentication_failed"
FAILED
test defect_used_proposal_identifier_cannot_be_re_registered_after_gc ...
thread 'defect_used_proposal_identifier_cannot_be_re_registered_after_gc' panicked at review.rs:189:5:
assertion `left == right` failed: used proposal IDs remain single use
  left: "ACCEPTED"
 right: "ipc/duplicate_identifier"
FAILED

failures:

failures:
    defect_certificate_expiry_during_live_proposal_prevents_signing
    defect_definitive_naming_error_does_not_erase_still_nameable_keys_without_status
    defect_pending_compromise_is_not_cleared_by_old_rotated_retry
    defect_rechecks_authority_after_unlock_before_signature
    defect_recovery_retry_authenticates_passphrase_before_naming
    defect_used_proposal_identifier_cannot_be_re_registered_after_gc

test result: FAILED. 11 passed; 6 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.55s

error: test failed, to rerun pass `--lib`
```
