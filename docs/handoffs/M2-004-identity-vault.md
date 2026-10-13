# Handoff: M2-004 — identity and authority registry, encrypted vault, exact-proposal signing

- **State:** M2-004 is **IMPLEMENTED → IN_REVIEW**. The author approves nothing.
- **Role:** IMPLEMENTER. **Model/provider:** Claude Code (Anthropic), cloud VM. Requested reviewer: Codex (independent).
- **Branch:** `codex/m2-cryptographic-contract-core` (pushed fast-forward).
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md), M2-004 (A04, A05, A12 in-process); [ADR 0005](../decisions/0005_m2_identity_vault_ipc.md) (G3), [ADR 0006 §§5, 10](../decisions/0006_m2_storage_anchor_recovery.md), [ADR 0008 §4](../decisions/0008_m2_retention_privacy_update_identity.md), [service IPC v1](../specs/m2_service_ipc_v1.md) §§1–4.
- **Base:** 20c849b22ff7cb56c9266d447a28d6c692cbe532 (M2-003 DONE).
- **Candidate:** **bb3f1ba** (final implementation commit; full hash in the verification table). The range 20c849b..bb3f1ba has three implementation commits:
  - 0e4f993172019de8a55badbdd8a0a56943b89eb1: the implementation;
  - e5be2065d10c1f41bac27020540117175ff46d5b: two static-gate fixes found by the clean-clone static checks at 0e4f993 (yamllint line length in the workflow comment; the protection check script's executable bit);
  - bb3f1bad77080da34069af62c8d2cc390f2fcf7e: the protection probe's locked-mapping check, after the first CI run of the positive path. The vault's protection applied, but the probe compared smaps `Locked` (proportional, PSS, for shared pages) with `Rss`. It now requires the VM_LOCKED (`lo`) flag on every resident mapping.
- **Docs-only follow-up:** the commit adding this handoff, the evidence, the task progress and STATUS.

## Completed work

### identityd (`core/identityd`, safe Rust)

| Module | What it does |
|---|---|
| `caller` | Roles of IPC §1 from kernel-reported peer credentials only: service users, `trusted_client` (regular UID **and** GID `_cham3-ipc`), `admin` (UID 0), `fixture_consumer`. The account configuration is validated (distinct, non-root, non-regular accounts; non-zero IPC group), otherwise refused. An ordinary same-UID process gets no role. |
| `outcome` | One `layer/code` per refusal: layers 1–5 from `cham3_contract_format`, layer-6 codes, registry-rule codes (`admission`), the IPC §4 vocabulary, and the `denied`/`error` status of IPC §2. |
| `index` | Per-user key-authority state over journal records in order: enrollment, certificates, invalidations, and the registry records that change key authority (`containment`, `quarantine`, `admin_recover`, `recovery_opened`, `recovery_keys`, `recovery_closed`, `deletion_pending`). It answers `resolve_key` with a scope (`normal` / `recovery` / `denied(code)`), the layer-6 commit check in the oracle's order (actor binding, key validity, class, window, then user state), and the ceremony scope check used by signers. |
| `directory` | All users. It verifies identity objects. Certificates and invalidations are verified under the user's registered identity key only. A self-signed enrollment's candidate key comes from a strict decode, then the object is verified from scratch under that key. |
| `journal` | identityd keeps no store (R05). It appends through registryd `append_identity` and rebuilds by paging `identity_state` (16 records a page). `memory_journal_t` is a **labeled M2-004 fixture** for registryd (M2-005/006): no files, anchor or crash points. |
| `service` | Operations with role checks: `register_enrollment`, `register_certificate`, `register_invalidation` (`service:vaultd` only), `resolve_key` (`service:*`, `fixture_consumer`), `user_identity` (`service:*`; `trusted_client` for its own UID). The journal is refreshed before every answer. A journal that cannot be read, or a record that does not apply, denies that user's keys (`storage_unavailable`) until a rebuild succeeds. An invalidation denies the key in memory **before** any journal access; a failed append answers `restriction_not_durable` and keeps the denial. |

The **M2-004 obligation** from the M2-003 review is met: a certified `subject_public_key` must be a canonical point that is not of small order, otherwise `key_authority/key_not_certified`. An enrolled key that is a bad point cannot verify its own enrollment.

### vaultd (`core/vaultd`, safe Rust)

| Module | What it does |
|---|---|
| `kdf` | Argon2id, with memory 64 MiB–1 GiB, passes 1–10, lanes 1–4 and the locked budget (default 512 MiB) checked **before** derivation. The release calibration is 256 MiB / 3 passes / 1 lane. `kdf_workspace_t` owns the `Block` workspace: exact size, zeroized after every derivation on success and every error path, and again on drop. A regression control shows argon2 itself leaves it populated. The KEK goes straight into a `secret_region_t`. |
| `key_object` | The ADR 0005 §4 object as strict `cham3_dcbor_v1`. The AAD is the encoding of every field except `ciphertext`/`tag`. XChaCha20-Poly1305 runs in place in secret regions. After decryption `key_id(public(seed)) == key_id` is required (substitution detection). |
| `store` | `<root>/<uid>/<key_id hex>.key`, directories `0700` and files `0600`. The vault UID must own them, regular files only, no symbolic links, and the opened inode must match. Writes: temp, fsync, rename, directory fsync. Erasure: overwrite with zeros, fsync, unlink, directory fsync (SSD/journaling limit stated). |
| `records` | Pending registration bundles (signed public objects, held key ids, keys to erase afterwards) and the persistent failed-attempt counter (`2^(n−5)` s after five failures, cap 1 h). |
| `service` | Operations below. There is no unlocked state, no raw-signing operation and no key export. |
| `protection` | At start: `RLIMIT_CORE=0` and `PR_SET_DUMPABLE=0`, then a memlock limit of budget + 256 MiB, then `mlockall(CURRENT|FUTURE)`. Any failure is `protection_unavailable`. |
| `links` | The identityd and registryd operations vaultd calls, always as `service:vaultd`. The in-process adapter is for M2-004; the M2-007 transport implements the same traits. |

Vault operations:

- `register_proposal` (`service:contractd`). The vault decodes and checks the payload itself (layers 4–5 under the proposal's `kid`) and binds:
  - `actor.uid` to `owner_uid`;
  - the trunk parent to `expected_head`;
  - `policy.epoch` to `policy_epoch`.

  Then it checks layer 6 and the scope with identityd, requires a stored key object that matches the certified key, and returns `SHA-256(to_be_signed)`. Limits: commits only; expiry 1–120 s; at most 8 pending proposals per user and 64 in total; envelopes up to 61 440 bytes.
- `approve_and_sign` (owner's `trusted_client`):
  1. checks the owner, that the proposal is unused and unexpired (monotonic **and** UTC clocks), and the digest in constant time;
  2. checks the attempt limit;
  3. rechecks key status and scope;
  4. derives the KEK and decrypts into a secret region;
  5. checks expiry again;
  6. signs the Sig_structure the vault built, verifies it, and marks the proposal used.
- `cancel_proposal`: owner's `trusted_client` or `service:contractd`.
- `create_user_keys`, `rotate_key`, `invalidate_key`, `recover_keys` (owner's `trusted_client`):
  - new objects and a **pending bundle** are durable before anything is registered;
  - a transient failure keeps everything; a definitive refusal erases only the bundle's new keys;
  - a retry resumes with the held keys and needs their passphrase;
  - rotation invalidates (`rotated`) the class's usable keys first;
  - invalidated, rotated and retired objects are erased (ADR 0008 §4).
- `recover_keys` reconciles from `recovery_status` (IPC §3, R14):
  - unknown status: keep everything;
  - the held keys named: done;
  - other keys named: erase the held new keys only;
  - nothing named and the ceremony open: resume naming with the held keys (idempotent);
  - the ceremony gone: erase the held new keys.

### Tests and evidence tooling

- **Rust: 114 + 114 cargo tests** (debug and release), inventory-exact. New this package:
  - identityd 22: roles, vocabulary, UTC arithmetic, registration, user separation, restrictive invalidation, points, windows, rotation rules, fail-closed journal, paging, layer 6, ceremony scopes, proof of possession;
  - the scenario differential `journal_scenarios`: **every `key_authority` journal scenario of the published vectors (12) step by step: 318 comparisons, 0 disagreements** with the independent oracle's layer-6, record, naming, status and key-state observations;
  - vaultd 29: KDF bounds/budget/workspace, records, protection refusal, key creation, exact signing, approval binding/replay/expiry, registration checks, attempt limiting across restart, ten tampered or substituted object cases plus mode and symlink, no secret in files, invalidation and restored files, rotation, interrupted enrollment and invalidation, user states, roles, and five recovery tests (scope, lost anchor update + restart, unavailable status, abandoned ceremony, new ceremony past an abandoned attempt);
  - os_adapter +1 (memlock limits).
- **Independent key-object oracle** (`tests/m2/oracle/vault_object_oracle.py`, unit suite). It uses libargon2, libsodium, hashlib and the ECR oracle's encoder, and drives `cham3_vault_probe`. **38 checks:**
  - Rust-sealed objects for three classes and two users are byte-identical to the oracle's and open in it;
  - the oracle's object opens in Rust;
  - a wrong passphrase, 12 authenticated-field changes, 11 structural changes, 6 KDF bound/budget cases and a substituted seed are each refused with the expected code.
- **Harness: 133 tests** (130 carried):
  - `test_m2_identity_vault.py`: **14 seeded defects**, each in a disposable workspace copy, each caught by its named Rust test (not by the build), with the unmodified copy passing;
  - `test_m2_vault_protection.py`: the release-built `cham3_vault_protection_probe` exits 3 `protection_unavailable memlock_too_low` under an 8 MiB memlock limit.
- **Protection positive path:** `tests/m2/protection/check_vault_protection.sh`, run as a separate CI step in the pinned container with `--ulimit memlock=1073741824:1073741824` (no `--privileged`, no added capabilities). The qualification container's 8 MiB limit cannot be raised by an unprivileged process, and this VM's root cannot raise it either.

### Decisions made within the approved design (for review)

1. `identity_state` pages carry the user's identity objects **and** the registry records that change key authority, with journal sequences. IPC §3 says "identity records"; the record kinds are those of the published scenarios.
2. A second enrollment for a UID is `ipc/duplicate_identifier`. The certified-key point failure is `key_authority/key_not_certified`; wire §7 has no dedicated code.
3. Certificate windows: `not_after` must follow `not_before`; temporary certificates need `not_after` at most 7 days later (`certificate_window`). `supersedes_key_id` must name an earlier key of the user in the same class (`key_not_certified`).
4. **Differences from the oracle model, both unexercised by the vectors:**
   - An invalidation is accepted while quarantined, per IPC §3 "every user state except deleted". The model refuses it. vaultd cannot sign one then, because `resolve_key` denies the identity key (ADR 0006 §4).
   - A later `compromised` invalidation escalates an earlier reason; the model keeps the first.
5. A deleted user registers nothing, not even an identical retry.
6. `user_identity` also returns the open ceremony (`recovery_id`, key epoch, named keys), which `recover_keys` needs.
7. Vault error mapping:
   - corrupt or unsafe stored objects and KDF parameters outside the bounds are `storage_unavailable`;
   - over-budget KDF or secret-memory failure is `protection_unavailable`;
   - changed authenticated metadata is indistinguishable from a wrong passphrase (`authentication_failed`) and counts as an attempt.
8. Object times (`enrolled`, `issued`, `not_before`) come from the system UTC clock. ADR 0007 trusted-time integration is M2-006/M2-007.

### Not in M2-004 (recorded sequencing)

- **`replace_identity`** (admin): needs a journal record format and a layer-6 code for a replaced identity's keys; wire §7 has none. It moves to M2-006 with `recover_user` and deletion.
- **Vault-side deletion cleanup** (ADR 0008 §4): the IPC spec has no vaultd route for it yet. Moves to M2-006.
- **registryd:** the identity journal and the ceremony operations used in tests are labeled fixtures. M2-005/006.
- **contractd:** `prepare_proposal` and `submit_signed`, M2-005.
- **M2-007:**
  - sockets, `SO_PEERCRED` mapping at accept, framing and flooding limits;
  - the service binaries, the setgid client with terminal passphrase custody, and systemd units with `LimitMEMLOCK`.

## Verification

| Check | Environment/command | Result | Evidence/limits |
|---|---|---|---|
| Author amd64 qualification at bb3f1bad77080da34069af62c8d2cc390f2fcf7e | Clean UID-1000 clone, pinned container, `qualify_m2_rust.sh` with `CHAM3_M2_REQUIRE_CLEAN=1`; no `--privileged`, no added capabilities, no `--record` | PASS. Unit **114 + 114** cargo tests. Provider oracle 7/7. ECR 19/96/18 (22 scenarios / 346 steps). Format differential 5 950 cases / 0 mismatches. **vault_object_oracle 38/38**. Harness **133** (14 seeded identity/vault defects detected; protection refusal) | [m2_004_amd64_vm_bb3f1ba](../evidence/M2-cryptographic-contract-core/m2_004_amd64_vm_bb3f1ba/) |
| Layer-6 differential | `journal_scenarios` against every `key_authority` journal scenario of the published vectors | PASS: 12 scenarios, 318 comparisons, 0 disagreements. A planted defect (minimum key epoch not enforced) gives 2 disagreements | `unit/cargo_test_*.json`; harness seeded defects |
| Static checks / static tests | Same clone and container: `run_static_checks.sh`, `tests/run_tests.sh static` | PASS / 82 OK; the clone was clean afterwards | `static_checks.log`, `static_tests.log` |
| Vault protection, refusal | Release probe under an 8 MiB memlock limit | PASS: `protection_unavailable memlock_too_low`, exit 3, nothing derived | `protection.txt` §1; harness |
| Vault protection, positive path | Pinned container with `--ulimit memlock=1073741824:1073741824` | **PASS in CI** (m2_checks 38341512299): non-dumpable, core limit 0, 0 unlocked mappings, `[vdso]` special, 4 dont-dump mappings, release derivation 431 ms. **BLOCKED in this VM** (the Docker daemon and root cannot raise memlock) | `protection.txt` §§2–4; [CI evidence](../evidence/M2-cryptographic-contract-core/m2_004_amd64_ci_bb3f1ba/) |
| GitHub Actions at bb3f1ba | m2 38341512299, m1 38341511647, m0 38341511661 | PASS, every job and step | CI evidence; the artifact ZIP was not downloaded (author `gh` unauthenticated) |
| Earlier attempts 0e4f993 and e5be206 | Same gates | FAIL: static gates at 0e4f993; the positive-path probe check at 0e4f993 and e5be206. Fixed by e5be206 and bb3f1ba | `attempts.md` |
| Miri, sanitizers, coverage-guided fuzzing | — | NOT_RUN (NOT_AVAILABLE on the pinned toolchain) | Bounded seeded and differential tests only |
| IPC, service processes, registryd admission, human presence | — | NOT_RUN | M2-005..M2-007; M5 |

## Secret custody (who clears what)

| Secret | Held in | Cleared by |
|---|---|---|
| Passphrase | caller's `secret_region_t` (the M2-007 transport receives into one) | region drop (volatile zeroization) |
| KEK | `secret_region_t` | region drop |
| Seed | `secret_region_t` (generated by `getrandom` into it, or decrypted in place) | region drop |
| Argon2 workspace | `kdf_workspace_t` | explicit clear after every derivation and on drop |
| ed25519-dalek `SigningKey` copy | library | zeroized on drop (`zeroize` feature, ADR 0003) |
| XChaCha20-Poly1305 key copy | chacha20poly1305 0.10.1 | zeroized on drop (pinned source, `lib.rs` 292–297) |
| ChaCha20 keystream state | chacha20 0.9.1 | zeroized (feature enabled by chacha20poly1305) |
| Poly1305 one-time MAC key | poly1305 0.8.0 | **not** zeroized (its `zeroize` feature is off). It is a library temporary, covered only by `mlockall`, non-dumpable state and `RLIMIT_CORE=0` |
| Stack temporaries in the libraries | — | not erased; same coverage. Abort or death bypasses every destructor (ADR 0003 §6) |

## Remaining

- **Known risks and limits:**
  - in-process composition only, with labeled fixtures for registryd;
  - passphrase knowledge with exact-digest binding is **not** a trusted display, trusted input path or proof of human presence (ADR 0005 §8);
  - same-UID isolation is M3;
  - the attempt counter is not rollback-protected (ADR 0006 §5);
  - Miri, sanitizers and fuzzing are NOT_RUN.
- **Uncommitted state:** none after the evidence commit.
- **Next:**
  1. Codex reviews M2-004.
  2. After an APPROVE, M2-004 is DONE and M2-005 starts.
- **Reminder for the end of M2:** the final review uses the Codex Security plugin skill. The plugin has been BLOCKED in both reviewer environments so far; this review should try it again.

## Review prompt for Codex

~~~text
You are the independent REVIEWER (Codex) for cham3 M2-004 in Kayyo321/cham3. You did not author this work.
Read AGENTS.md, docs/tasks/M2-cryptographic-contract-core.md (M2-004, A04, A05, A12), ADR 0005 (G3), ADR 0006 sections
3-5 and 10, ADR 0008 section 4, docs/specs/m2_service_ipc_v1.md, docs/specs/m2_wire_format_v1.md section 7 and
docs/handoffs/M2-004-identity-vault.md. Treat author claims and evidence as claims to verify; judge against the
approved ADRs and specifications.

Targets: candidate bb3f1bad77080da34069af62c8d2cc390f2fcf7e (range 20c849b22ff7cb56c9266d447a28d6c692cbe532..bb3f1ba: the
implementation 0e4f993 plus two fix commits e5be206 and bb3f1ba, see the handoff) and the docs-only commit after it on origin/codex/m2-cryptographic-contract-core; confirm that commit changes only docs/.
Do not merge main or review branches into the implementation branch; do not edit implementation files.

1. Scope and boundaries: identityd/vaultd are safe Rust with #![forbid(unsafe_code)]; the only os_adapter change is
   the memlock limit query/raise through safe rustix; Cargo.lock gains only dependency edges (no new crate or version);
   the two probe binaries are qualification tools only. Judge the recorded sequencing (replace_identity and vault-side
   deletion cleanup to M2-006; registryd, contractd and transport to M2-005..M2-007) against the task file.
2. identityd: roles only from peer credentials (core/identityd/src/caller.rs); layer-6 order and codes
   (index.rs check_commit_authority, scope_of) against wire section 7 and ADR 0005 section 2; the scenario
   differential core/identityd/tests/journal_scenarios.rs (every key_authority journal scenario; check that it compares
   what it claims and cannot pass vacuously); registration rules, including the M2-003 obligation
   (public_key_is_valid on certified subject keys), self-signed enrollment verification (directory.rs), restrictive
   invalidation ordering and restriction_not_durable (service.rs), fail-closed journal handling and rebuild. Judge the
   handoff's "Decisions made within the approved design" 1-8, including the two differences from the oracle model.
3. vaultd: key object format and AAD (key_object.rs), KDF bounds/budget before derivation and workspace clearing on
   every path (kdf.rs), file store checks and atomic writes (store.rs), register_proposal binding and checks,
   approve_and_sign (digest, owner, expiry by both clocks, single use, attempt limit, key status recheck), rotation,
   invalidation, pending bundles and recover_keys reconciliation (service.rs) against service IPC section 3 / R14,
   process protection order and fail-closed behavior (protection.rs). Look for any path that signs caller-chosen bytes,
   signs without a fresh passphrase for that proposal, signs twice, exports a seed, erases a key that can still be
   named, or lets one user's caller act for another.
4. Secret custody: check the handoff's cleanup table against the pinned sources (/usr/share/cargo/registry:
   chacha20poly1305-0.10.1 Drop, chacha20-0.9.1 zeroize, poly1305-0.8.0 without zeroize, ed25519-dalek 2.1.1, argon2 0.5.3)
   and that seeds, KEKs and passphrases live only in secret_region_t.
5. Evidence: in a clean clone at bb3f1ba owned by an unprivileged UID, build the pinned image and run the qualification
   (docker run --user "$(id -u):$(id -g)" ... bash build/scripts/qualify_m2_rust.sh with CHAM3_M2_REQUIRE_CLEAN=1; no
   --privileged, no added capabilities, no --record) and confirm: unit 114+114 cargo tests, provider oracle 7/7, ECR
   oracle 19/96/18 with 22 scenarios / 346 steps, format_differential 5950 cases / 0 mismatches, vault_object_oracle
   38 checks PASS, harness 133 (including 14 identity/vault seeded defects); then build/scripts/run_static_checks.sh and
   sh tests/run_tests.sh static. If your Docker allows it, also run the protection positive path:
   docker run --rm --user "$(id -u):$(id -g)" --ulimit memlock=1073741824:1073741824 -e HOME=/tmp/cham3_home
   -v <clone>:/work -w /work cham3-m2-dev sh tests/m2/protection/check_vault_protection.sh out/m2/protection
   (otherwise record BLOCKED and compare the CI step). Write and run your own adversarial tests against the candidate
   (cross-user, role, replay, forged/stale approval, tampered or substituted key objects, malformed identity objects,
   ceremony and reconciliation sequences, journal faults); record exact commands and results.
6. Compare GitHub Actions m2_checks 38341512299, m1_checks 38341511647 and m0_checks 38341511661 for bb3f1ba (artifact:
   gh run download 38341512299 -n m2_qualification_amd64), including the "Vault process protection, positive path" step.
7. If the Codex Security plugin skill is available in your environment, run it on the candidate range and record its
   exact name/version, commands and findings; if not, record BLOCKED with the reason (it remains required for the
   final M2-008 review).

Record every check as PASS/FAIL/NOT_RUN/BLOCKED with exact commands, environment and evidence. Return APPROVE or
CHANGES_REQUESTED for M2-004; make no owner decisions. Publish only your review record on a new review branch
(review/m2-004-security), and end your reply with this block filled in:

=== CHAM3 M2-004 VERDICT ===
reviewer: Codex
environment: <machine/OS/arch; UID; Docker version; network notes>
review_record: <branch> @ <commit> : <path> (sha256 <digest>)
candidate_reviewed: bb3f1bad77080da34069af62c8d2cc390f2fcf7e
docs_commits_reviewed: <shas> (docs-only: yes/no)
verdict: M2-004: <APPROVE/CHANGES_REQUESTED>
scope_and_boundaries: <PASS/FAIL> (<notes>)
identityd: <PASS/FAIL> (<layer 6, differential, registration, obligation, invalidation ordering>)
vaultd: <PASS/FAIL> (<key objects, KDF, store, proposals/approval, rotation/invalidation, recovery reconciliation, protection>)
secret_custody: <PASS/FAIL>
author_decisions: <1-8 accepted / objections>
qualification_amd64: <PASS/FAIL/BLOCKED> (<counts>)
protection_positive_path: <PASS/FAIL/BLOCKED> (<own run or CI step>)
static_checks: <..>   static_tests: <..>
own_tests: <what you ran and results>
security_plugin: <ran (name/version, findings) / BLOCKED (reason)>
ci_comparison: <..>
new_findings:
- id: <..> | severity: <..> | location: <file:line> | summary: <..> | required: <..>
notes_for_owner: <..>
=== END VERDICT ===
~~~
