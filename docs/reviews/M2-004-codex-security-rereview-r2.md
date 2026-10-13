# M2-004 revision 2 independent Codex security review

**Verdict: CHANGES_REQUESTED.** R01, R02, R03 and R05 pass. R04's original replay reproducer passes, but the new retention quotas do not hold across pending-to-retired transitions. One Low finding below requires correction and an exact-candidate re-review. I01 is documentation-corrected and acceptable against R14. Earlier PASS assessments remain intact within their recorded scope.

Reviewer: Codex, independent REVIEWER, 2026-10-13 UTC. No implementation files were edited; no branches were merged; no owner decision is made. Publication branch `review/m2-004-r2-security`, base `9cf882503848f8c6cc4938a588cc9fc062a9392d` (origin/main). Only this review record is published. Test sources and logs stay outside the candidate; the additional source is reproduced in this record's appendix.

## Exact targets, authority and environment

- Candidate `ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba`, tree `f3ae30934d6140910d44ba5f7357cd15e267e75a`, one commit with parent `d2c29ad7a09b2b1d49a23e9fa92c9656aa74c711`.
- Prior code candidate `bb3f1bad77080da34069af62c8d2cc390f2fcf7e`; prior independent review `f2d82b7a663c1579582bc5c88fe2832a6f97e061:docs/reviews/M2-004-codex-security-review.md`, verified SHA-256 `eb0b011ffbab9ae72002df1e264bc9f067c5d2d5d463a4733bed822148db596a`.
- Docs follow-up `acefb668976359d2b123b29246f86e717f41930a`, parent ca40c62. **PASS: docs-only**, independently checked with `git diff --name-only ca40c62 acefb66`. Its finding resolutions and revision-2 handoff were read using `git show`, without overlaying them onto the candidate.
- **PASS: range scope.** `git diff --name-only d2c29ad ca40c62` contains exactly the nine files listed below. `git diff --name-only bb3f1ba ca40c62` contains those same nine non-doc files **plus 37 docs paths from d2c29ad**; it is not literally a nine-file range. The author's `unchanged_inputs.txt` correctly excludes docs for its nine-file statement. No dependency, Cargo.lock, specification, ADR, vector, oracle or key-object format change.
- Clean detached clone `/workspace/m2-r2-candidate`, cloned with `--no-hardlinks` from the fetched repository; directory and .git owner UID/GID 1000, mode 0700. Source identity checks report `dirty_tracked 0`. Evidence `/workspace/m2-r2-evidence`; all synthetic key generation is inside disposable test containers.
- Outer machine `d0fb1765f2cd`, Debian 13.6, Linux 6.18.44, x86_64, UID/GID 1000 (agent). Docker client/server 28.4.0, vfs, runc, seccomp enabled. Managed networking is restricted/enforced through the configured proxy, no VPN, secrets or outbound identities. GitHub Git fetch/push and Debian package fetch work. api.github.com is Forbidden to the CLI; connected GitHub tools work. The returned artifact File Service URL is HTTP 403 to this executor; no proxy/TLS bypass was attempted.
- Built `cham3-m2-r2-dev`, image `sha256:c9ffb27383b7feb76a048127aa0dffa7df277afbf2492e4cdb0b4fa31ed7c84c`, linux/amd64. Unchanged pinned base `debian@sha256:913f6706df59a68922d1dd08f78c2476560a8d367897200a6005b00e5f67c2d5`. Inside: Debian 13.7, UID/GID 1000, rustc/cargo 1.85.1, LLVM 19.1.7. Installed-package, downloaded-deb and per-file crate-source locks match.

Read authority: AGENTS.md, relevant PROJECT §§3–5/14/Phase 2, workflow, roles, orchestration, testing and security invariants; task M2-004 and its staged dependencies; ADR 0005 (including §2), ADR 0006 (including §§3, 5, 10), service IPC v1 §3 (identityd invalidation, vault rows, restrict and R14). The prior review, finding resolutions and handoff are evidence/claims, not substitutes for those approved requirements.

The nine revision files, all inspected: `core/identityd/src/index.rs`, `core/identityd/src/service.rs`, `core/identityd/tests/service_operations.rs`, `core/vaultd/src/service.rs`, `core/vaultd/tests/fixture/mod.rs`, `core/vaultd/tests/vault_operations.rs`, `core/vaultd/tests/vault_recovery.rs`, `tests/m2/harness/test_m2_identity_vault.py`, `tests/m2/test_inventory.txt`. Supporting directory/journal/caller/scope, key-object/KDF/store/records/links/clock/protection and secure-memory paths were followed where needed.

## Required change: M2-004-R2-01 — Low — pending transitions exceed retained-ID quotas

**FAIL**, R04 retention bounds. Location `core/vaultd/src/service.rs:723` (also :724; retirement at :874 and :900).

Registration separately counts pending and retired proposals. It refuses a new registration only when the **already retired** count reaches 32 per user or 256 globally. Existing pending entries can subsequently become used/cancelled without a capacity check or reserved slot. This exceeds the promised retained-ID bounds even with the default configuration, serialized legitimate operations, and no clock movement.

Independent debug and release reproductions:

1. Register/cancel 31 proposals for one user, then register eight pending proposals before cancelling any of them. Cancel all eight. Every call succeeds, leaving **39 retired IDs** with all original 120-second lifetimes live, against the configured bound 32.
2. Use nine enrolled synthetic users: 31 retired IDs each for the first eight, seven for the ninth, hence 255 globally. Register eight pending proposals each for the first eight users (64 total). Cancel them all. Every call succeeds, leaving **319 retired IDs**, against the configured global bound 256.

`r2_retired_per_user_quota_reserves_pending_transitions` and `r2_retired_global_quota_reserves_pending_transitions` each FAIL in debug and release; observed counts are 39 and 319 in both. The byte-release and duplicate-ID behavior themselves work. This is a bounded quota overshoot, **not unbounded memory growth, replay restoration or an unrestricted-signing bypass**; severity is Low. It still fails the explicit revision-2 quota requirement and makes the handoff's exact bounds inaccurate.

Required: account for every live pending ID's future retention when admitting proposals, or implement an equivalent invariant that keeps both limits while allowing safe completion/cancellation. Reject excess admission with `quota_exhausted`. Do not fix this by dropping a live used/cancelled marker, reviving its ID, or stranding already admitted workflows. Add per-user and global batch-transition controls (including approvals) to the regression suite, and re-review the exact correction.

## Prior finding reassessment

| Item | Status | Independent evidence and judgment |
|---|---|---|
| R01 | PASS | Original reproducer passes in both profiles. `settle_pending_denial` checks the durable directory, not the pending overlay; both already_present and successful append use it. A durable compromise covers all reasons; rotation/retirement cannot cover a pending compromise. New two-key tests exercise weaker new appends, reverse compromise retries, read unavailability followed by refresh/rebuild, and pending rotation followed by durable compromise. Reasons remain correct after a durable restart. |
| R02 | PASS | Original expiry reproducer passes. `check_signing_window` uses current UTC, validates both time strings and includes both bounds. It is invoked at registration and at both approval checks; historical layer 6 is unchanged. New tests cover start−1/start/end/end+1, extreme i64 times, malformed start and invalid-calendar end, no end, successful registration/signing exactly at not_before, and expiry crossing after unlock. Author's not_after positive and persistent-no-end controls also run in qualification. |
| R03 | PASS | Original clock-scheduled deletion reproducer passes. Final `check_signing_key` resolves status/scope/window and reads the stored object's binding before sign; refusal drops the secret region. New post-unlock tests reject missing object and journal unavailability. New rotation/invalidation interleavings reject deletion with no new signature/object/bundle registration; author recovery-abort test verifies no certificate registration. Remaining distributed serialization limit is stated accurately. |
| R04 | FAIL overall | Original cleanup/re-registration reproducer passes, as do owner/contractd cancellation, used-marker protection, two-clock lifetime expiry and documented restart limits. `retire` drops payload/protected Vecs and retains the marker. New batch tests fail the exact 32/256 bounds: M2-004-R2-01. |
| R05 | PASS | Original definitive naming-error reproducer and unknown-status/abort controls pass. Every initial registration/naming error enters pending reconciliation; the original error alone never erases objects. New definitive certificate-registration failures keep the bundle while status is unavailable or ceremony open, resume the held keys on recovery, and erase only the new keys after a positive abort status. Existing named-after-restart, other-named/abandoned and anchor-fault fixture controls remain in qualification. |
| I01 | PASS; docs corrected: yes | Held recovery completion behavior is unchanged and creates/signs nothing. Wrong-passphrase finalization remains ACCEPTED, so the unchanged experimental assertion still FAILs. This is acceptable against R14, not a new authentication finding. Enrollment retry is distinct: it unlocks the held identity object and requires the correct passphrase; the new enrollment control confirms wrong-passphrase refusal before any enrollment append. Both code documentation and revision-2 handoff describe this difference correctly. |

The new `interleaving_identity_t` answers from the real in-process identity service, then applies a one-shot change after a chosen resolve answer, retaining the old returned snapshot. This brackets derivation and demonstrates why a later sample is needed. It is a valid deterministic component fixture, not evidence of real sockets, threads or globally atomic signing. The tests assert the hook actually ran and count both resolutions; no-change verifies a real signature. The seeded defects remove the relevant checks and must cause the named test to fail, not merely fail compilation. The separately rerun original clock scheduler places denial after unlock and before sign.

## Carried obligations and earlier PASS items

**PASS: obligations accurately recorded and acceptable for staged M2-004**, without waiving later gates:

- M2-006 restriction ledger: identityd's pending denial is memory-only. Our test restarts identityd alone before durability and observes the loss of that overlay, then verifies vaultd's durable pending bundle blocks further signing work until the invalidation is retried durably. Other services between restart and retry see only durable state. ADR 0006 §3's ledger and conservative restart containment remain required; the current vault bundle does not replace them.
- M2-006/M2-007 cross-service serialization: final resampling removes the demonstrated long KDF gap; a restriction after the final answer can still cross a signature. The handoff explicitly preserves this obligation and does not claim distributed linearizability.
- M2-005 duplicate request IDs: proposal markers and pending proposals disappear together on vault restart. Our test verifies both facts. Admission must independently reject replay/duplicate request IDs; lifetime reuse of a proposal ID is not an admission guarantee.

| Earlier PASS assessment | Status | Evidence/limits retained |
|---|---|---|
| Scope, safe Rust and boundaries | PASS | No new dependency, unsafe adapter, deployment, IPC process, specification or crypto format. Domain services remain in-process; probe custody paths remain qualification-only. |
| Roles and user separation | PASS | Fixed peer role mapping unchanged; original role matrix, credential edges, cross-user signing/substitution controls pass. Real authenticated sockets NOT_RUN until M2-007. |
| Layer-6 ordering and differential | PASS | Historical actor/key/status/epoch/class/timestamp/scope checks unchanged. Direct independent run: 12 scenarios, 318 comparisons, 0 disagreements, two tests passed. This compares the established projection, not real admission/anchors. |
| Registration and M2-003 obligation | PASS | Enrollment/certification point validation and proof verification unchanged; bad points, malformed proof, wrong-user and deleted-user controls pass. No point-validation weakening. |
| Key objects/AAD | PASS | Format unchanged; independent oracle 38 checks PASS, plus original tamper/substitution controls. Metadata remains authenticated; decrypted public-derived key ID recomputed. |
| KDF | PASS | Parameters/budget checked before allocation; fixed owned workspace clears on success/error/drop; pinned library regression control remains. Same 256 MiB/3/1 calibration, 512 MiB budget, 256 MiB headroom. |
| Store | PASS within prerequisites | Owner/mode/type/symlink/inode, bounded-read and temp/fsync/rename/directory-fsync code unchanged; single vault writer and protected directories remain prerequisites. Erasure does not promise removal of old SSD/journal/COW blocks. |
| Protection | PASS for probe | Own raised-memlock run and independent CI positive path agree; low-memlock refusal is in qualification. Actual systemd/service/client integration remains M2-007. |
| Secret custody | PASS within prior limits | Feature trees and pinned source rechecked: dalek/ChaCha/AEAD cleanup remains enabled; Poly1305 zeroize remains absent, covered only by process protection as previously documented. Argon2 caller workspace belongs to vaultd. Library temporaries, abort/death and privileged/kernel compromise limitations remain unchanged. |
| Author decisions 1–8 | PASS within staged scope | 1: replay identity records remain an in-process extension with wire bounds deferred; 2: duplicate/point error mapping unchanged; 3: certificate window/predecessor rules unchanged; 4: compromise escalation now preserved across pending weaker retries (R01); 5: deleted state preempts registration retry; 6: ceremony epoch/named keys returned; 7: bounded storage/KDF/AEAD errors unchanged; 8: system UTC remains staged, with trusted-time integration later, while current signing window now checked. |

Miri, sanitizers and coverage-guided fuzzing: **NOT_RUN**, unavailable on the pinned stable/package toolchain. Real IPC/flooding, terminal custody/human presence, systemd startup, registry admission/anchor/ledger/crash/replacement/deletion and full OS enforcement: **NOT_RUN**, outside the implemented M2-004 scope. Component fixtures and successful signatures are not authorization or enforcement evidence.

## Commands and execution evidence

All shell checks ran as outer UID 1000. Candidate commands use the built pinned image; every run has `--user "$(id -u):$(id -g)"`; none uses `--privileged`, added capabilities or qualification `--record`. Logs are under `/workspace/m2-r2-evidence`. Repository mounts for adversarial/differential/static checks are read-only. Qualification/protection write only ignored outputs, and tracked source remains clean.

```sh
# Fetch/clone and identity; initial cwd /workspace/cham3
git fetch origin refs/heads/codex/m2-cryptographic-contract-core:refs/remotes/origin/codex/m2-cryptographic-contract-core refs/heads/review/m2-004-security:refs/remotes/origin/review/m2-004-security
git clone --no-hardlinks /workspace/cham3 /workspace/m2-r2-candidate
git -C /workspace/m2-r2-candidate checkout --detach ca40c62ed7a5fc29ba8910fef514f3ce29ea4fba
git show f2d82b7:docs/reviews/M2-004-codex-security-review.md
git show acefb66:docs/reviews/M2-004-finding-resolutions.md
git show acefb66:docs/handoffs/M2-004-rereview-r2.md

# cwd /workspace/m2-r2-candidate
git show --format='%H%n%P%n%T' --no-patch ca40c62
git show --format='%H%n%P' --no-patch acefb66
git diff --name-only bb3f1ba ca40c62
git diff --name-only d2c29ad ca40c62
git diff --name-only ca40c62 acefb66
git diff --check d2c29ad ca40c62
git diff --exit-code bb3f1ba ca40c62 -- Cargo.toml Cargo.lock .cargo core/vaultd/src/key_object.rs core/vaultd/src/kdf.rs core/vaultd/src/store.rs core/vaultd/src/protection.rs core/native core/contract_format build .github docs/specs docs/decisions tests/m2/vectors tests/m2/oracle tests/m2/vault_object_oracle tests/m2/protection
git status --porcelain
stat -c '%u:%g %a %n' /workspace/m2-r2-candidate /workspace/m2-r2-candidate/.git
id
uname -a
cat /etc/os-release
cat /etc/codex/network-policy.json
env -u DOCKER_HOST -u DOCKER_CONTEXT -u DOCKER_TLS -u DOCKER_TLS_VERIFY -u DOCKER_CERT_PATH docker --host=unix:///var/run/docker.sock info

docker build -f build/m2/dev_container.dockerfile -t cham3-m2-r2-dev build/m2
docker image inspect cham3-m2-r2-dev --format '{{.Id}} {{.Os}}/{{.Architecture}}'
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -e CHAM3_M2_REQUIRE_CLEAN=1 -v /workspace/m2-r2-candidate:/work \
  -w /work cham3-m2-r2-dev bash build/scripts/qualify_m2_rust.sh
docker run --rm --user "$(id -u):$(id -g)" --ulimit memlock=1073741824:1073741824 \
  -e HOME=/tmp/cham3_home -v /workspace/m2-r2-candidate:/work \
  -w /work cham3-m2-r2-dev sh tests/m2/protection/check_vault_protection.sh out/m2/protection
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -v /workspace/m2-r2-candidate:/work:ro -v /workspace/m2-r2-evidence:/evidence \
  -e CHAM3_OUT_DIR=/evidence/static-out -w /work cham3-m2-r2-dev \
  bash build/scripts/run_static_checks.sh
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -v /workspace/m2-r2-candidate:/work:ro -v /workspace/m2-r2-evidence:/evidence \
  -e CHAM3_OUT_DIR=/evidence/static-out -w /work cham3-m2-r2-dev \
  sh tests/run_tests.sh static
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -e CARGO_TARGET_DIR=/evidence/differential-target \
  -v /workspace/m2-r2-candidate:/work:ro -v /workspace/m2-r2-evidence:/evidence \
  -w /work cham3-m2-r2-dev cargo test --frozen -p cham3_identityd \
  --test journal_scenarios -- --nocapture
```

| Execution | Status / exit | Evidence |
|---|---|---|
| Clone/identity/range checks | PASS / 0 | Candidate/parents/tree above; unchanged-range diff empty; docs-only follow-up; clean tracked source. |
| Pinned image build | PASS / 0 | `docker-build.log`; fresh package install, exact locks match in qualification. |
| Qualification | PASS / 0 | `qualification.log`: every required gate PASS, including clean source, pins/source locks, fmt/clippy/naming, debug/release/profile/backends, valgrind and both suites. Valgrind secure-memory check: 10 tests, zero errors (child fault processes not instrumented). |
| Unit | PASS | 121 debug + 121 release, inventory exact, no ignored required tests; provider 7/7; ECR 19 valid / 96 invalid / 18 admission, 22 scenarios / 346 steps; format differential 5950 cases / 0 mismatches; vault oracle 38 checks PASS. |
| Harness | PASS / 0 | 133 Python tests; all 20 seeded identity/vault defects detected through named test failures, unmodified controls pass; low-memlock refusal passes. `python_harness.json` reports exit 0 and the required test names; the SeededDefects list independently contains 20 entries. |
| Protection positive | PASS / 0, own run | `protection-positive.log`: required bytes 805306368, memlock 1073741824, derivation 548 ms, dumpable 0, core limit 0, unlocked mappings 0, special [vdso], dontdump mappings 4, protection_verified, exit_status 0. |
| Static checks | PASS / 0 | `static-checks.log`: shellcheck 25 files, Python 67, YAML 5, secret scan, all PASS. |
| Static tests | PASS / 0 | `static-tests.log`: 82 tests, 78.565 s, OK. |
| Direct layer-6 differential | PASS / 0 | `journal-scenarios.log`: two tests, 12 / 318 / 0. |
| Unchanged 17 tests, debug | FAIL / 101; expected I01 only | `prior-debug-locked.log`: 16 pass / 1 fail, 72.67 s, zero ignored/filtered. Initial lock-generation/debug run also 16/1, 76.03 s (`prior-debug.log`). |
| Unchanged 17 tests, release | FAIL / 101; expected I01 only | `prior-release.log`: 16 pass / 1 fail, 3.71 s, zero ignored/filtered. |
| Additional 12 tests, debug | FAIL / 101 | `new-debug.log`: 10 pass / 2 quota failures, 95.92 s, 17 original tests filtered deliberately. |
| Additional 12 tests, release | FAIL / 101 | `new-release.log`: same 10/2 result, 4.71 s, same deliberate filter. |
| Secret custody feature/source recheck | PASS / 0 after tool fallback | `secret-custody-grep.log`; initial attempt BLOCKED / 127 because rg is absent inside the image (`secret-custody.log`), repeated with grep. |

The original and additional external packages inherit the candidate backend config; the source fixtures are imported, not patched. Release cargo tests use the unwind test harness; the separate release protection probe exercises the deployed panic=abort profile. The additional tests use real minimum-approved Argon2id 64 MiB/1/1, real AEAD and Ed25519, synthetic passphrases, and labeled journal/registry/clock fixtures.

Secret-custody command used the same read-only candidate image/user and ran `cargo tree --frozen -p cham3_vaultd -e features -i poly1305`, then the same command for `chacha20`, then `grep -n -E "impl Drop|zeroize"` on `/usr/share/cargo/registry/{ed25519-dalek-2.1.1/src/signing.rs,ed25519-dalek-2.1.1/src/hazmat.rs,chacha20poly1305-0.10.1/src/lib.rs,chacha20-0.9.1/src/lib.rs,poly1305-0.8.0/src/backend/soft.rs,argon2-0.5.3/src/lib.rs}`. The pinned source manifest in qualification verifies the underlying files, not merely crate names. The provider oracle retains the explicit NOT_CHECKED for libargon2's RFC secret/AD interface; the Rust KAT and independent ordinary-input Argon2 comparison pass.

## CI comparison and artifact limitation

**PASS** for exact candidate identity, every job/step outcome and independently retrieved complete text logs. **BLOCKED** for ZIP/per-file artifact inspection.

Exact connected GitHub calls (repo_full_name `Kayyo321/cham3`): `github_fetch(url="https://api.github.com/repos/Kayyo321/cham3/actions/runs/<id>")` and `github_fetch_workflow_run_jobs(run_id=<id>)` for **38347259191**, **38347259188**, **38347259258**. Each head_sha is ca40c62, status completed, conclusion success, push event. `github_fetch_workflow_job_logs(job_id=<id>)` fetched all seven jobs: **115131427974**, **115131427820**, **115131427985**, **115131428080**, **115131428041**, **115131428364**, **115131428412**. Logs are retained as `ci-log-<id>.txt`; metadata as `ci-run-<id>.json`/`ci-jobs-<id>.json`.

- m2 qualify_amd64: qualification PASS; the **Vault process protection, positive path** step PASS. Log verifies requested unprivileged command/ulimit, dumpable 0, core limit 0, unlocked mappings 0, special [vdso], protection_verified, exit_status 0, derivation **471 ms**. Own run is **548 ms** with the same security observations. CI uses `--record`; the independent local qualification does not.
- m1 static_and_desktop: static checks, 82 static, 261 desktop, 16 staging, 22 native pin-identity tests PASS. privileged_capture: 77 tests PASS (CI disposable privileged suite only). package_build: package/Lintian and 8 tests PASS.
- m0 static_checks: checks and 82 tests PASS. image_build: package/live-image build, 17 image tests, throwaway VM boot to serial login and 18 boot-result tests PASS. package_build: package/Lintian and 8 tests PASS. No local M0/M1 privileged/full-image rerun is claimed.

Required CLI `gh run download 38347259191 -n m2_qualification_amd64 --repo Kayyo321/cham3 --dir /workspace/m2-r2-evidence/ci-gh` is **BLOCKED**, api.github.com Forbidden. `gh run view 38347259191 --repo Kayyo321/cham3 --json headSha,status,conclusion,jobs,url` is blocked identically. Connector fallback `github_fetch_workflow_run_artifacts(run_id=38347259191,name="m2_qualification_amd64")` independently returns artifact **11789878130**, 97312 bytes, not expired, ca40c62, advertised digest `sha256:6315b087535ee9bec503e2291ab3e01d76c4bd3ebbfa35fc4a7c7d07bb895a5d`.

`github_download_workflow_artifact(artifact_id=11789878130,file_name="m2_qualification_amd64.zip")` returns a File Service reference, but `curl --fail --location --output /workspace/m2-r2-evidence/ci-artifact.zip '<returned download_url>'` is **BLOCKED**, HTTP 403/exit 22. That is not a Sediment upload and is not eligible for download_file. The advertised digest is metadata, not a locally computed ZIP digest. Own counts are independently reproduced; CI logs confirm suite PASS, but a complete artifact inventory comparison is unavailable.

## Codex Security plugin attempt

Exact available skill: **codex-security:security-diff-scan**, package `skill://Plugin_1e648473be9c8191a91ac3947151af55/security-diff-scan`. **Version unavailable**: the provided metadata/front matter exposes no version, and no installed manifest is available. **BLOCKED**, no completed plugin scan and no plugin findings claimed.

`skills.read({"package":"c0/security-diff-scan"})` succeeds. Its mandatory `../../references/config-preflight.md` fails with "failed to read skill resource" at the plugin-root URI and the relative-traversal, skills/references, and codex-security/references alternatives. `skills.list({"authority":{"kind":"executor"}})` returns no skills. The callable tool inventory has no Codex Security scan/preflight/context methods. `rg --files / -g '*config-preflight.md' -g '*launch_codex_security_mcp*' -g '*plugin.json'` (permission errors suppressed) finds none; no terminal fallback scripts are available. The skill requires preflight before creating a scan goal/review workflow; no scan was created or replaced. This manual review is not labeled a plugin scan. The workflow remains required for final **M2-008** review.

## Independent test reproduction

The unchanged source/manifest were extracted byte-for-byte from the prior review's fenced **External Cargo.toml** and **External review.rs** sections. SHA-256 matches its record:

- `review.rs`: `922dd57b9ba92ce7b79517c6a3f890a1129626dc61766af6b814ee13df15bea4`.
- `Cargo.toml`: `82224a3c0e00b6cc3ac9c6b4ceaebc810c7d1912ff32366ecfdc8941ba961004`.
- New appendix alone (`new-tests.rs`): `e80537319a6426b2e26f1c5048e15eb47e4e3413bc8c171363df43994ddb0eb8`.
- Concatenated `new_review.rs`: `d619b29f72eea7f71600258a5e41c3f1f0609809c963f5cd690b912b51ea0b20`.

Prepare `/workspace/m2-r2-evidence/adversarial/.cargo/config.toml` by copying the candidate .cargo/config.toml. Copy its vectors.json to `adversarial/tests/m2/vectors/ecr_v1/vectors.json` (imported builders resolve CARGO_MANIFEST_DIR/../../tests). Put the unchanged manifest and review.rs in `adversarial/core/reviewer/`. For new tests, concatenate the unchanged review.rs and the appendix below into `adversarial/core/reviewer-r2/new_review.rs`; use the same manifest there with only `[lib] path` changed to `new_review.rs`. Each package initially generates its own external Cargo.lock using `--offline`; candidate source/lockfiles are never edited.

```sh
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -e CARGO_TARGET_DIR=/evidence/adversarial/target \
  -v /workspace/m2-r2-candidate:/work:ro -v /workspace/m2-r2-evidence:/evidence \
  -w /evidence/adversarial cham3-m2-r2-dev \
  cargo test --manifest-path core/reviewer/Cargo.toml --locked --offline \
  -- --nocapture --test-threads=1
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -e CARGO_TARGET_DIR=/evidence/adversarial/target \
  -v /workspace/m2-r2-candidate:/work:ro -v /workspace/m2-r2-evidence:/evidence \
  -w /evidence/adversarial cham3-m2-r2-dev \
  cargo test --manifest-path core/reviewer/Cargo.toml --release --locked --offline \
  -- --nocapture --test-threads=1
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -e CARGO_TARGET_DIR=/evidence/adversarial/r2-target \
  -v /workspace/m2-r2-candidate:/work:ro -v /workspace/m2-r2-evidence:/evidence \
  -w /evidence/adversarial cham3-m2-r2-dev \
  cargo test --manifest-path core/reviewer-r2/Cargo.toml --offline r2_ \
  -- --nocapture --test-threads=1
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home \
  -e CARGO_TARGET_DIR=/evidence/adversarial/r2-target \
  -v /workspace/m2-r2-candidate:/work:ro -v /workspace/m2-r2-evidence:/evidence \
  -w /evidence/adversarial cham3-m2-r2-dev \
  cargo test --manifest-path core/reviewer-r2/Cargo.toml --release --locked --offline r2_ \
  -- --nocapture --test-threads=1
```

The original initial debug invocation used the displayed original debug command with `--locked` omitted solely to generate its external lock. No candidate lockfile changed.

The new debug invocation is intentionally the initial offline lock generation; the new release invocation is locked. The unchanged original debug invocation was repeated locked after its first lock-generation run. Every failure above is an executed test assertion, not a compile failure.

## Additional source appendix

Save the following Rust fence verbatim as `new-tests.rs` and append it directly to the unchanged original review.rs.

```rust
// Additional independent revision-2 tests; synthetic fixtures, no source patches.
use std::cell::RefCell;
use cham3_vaultd::links::identity_client_t;
use cham3_identityd::index::{check_signing_window, key_resolution_t};
use cham3_identityd::outcome::{ipc_code_t, service_result_t};
use cham3_identityd::journal::append_receipt_t;
use cham3_identityd::service::{enrollment_receipt_t, user_identity_t};

#[test] fn r2_pending_multikey_read_fault_rebuild_and_stronger_durable() {
    let (mut i,u)=enrolled_identity();
    let keys=[u.persistent.key_id,u.temporary.key_id];
    let compromise:Vec<_>=keys.iter().enumerate().map(|(n,k)|builders::invalidation(&u.identity,k,invalidation_reason_t::compromised,&format!("c{n}"))).collect();
    i.journal_mut().faults.fail_appends=true;
    for o in &compromise { assert_eq!(code(i.register_invalidation(Vaultd,1001,o)),"ipc/restriction_not_durable"); }
    i.journal_mut().faults.unavailable=true;
    assert_eq!(code(i.resolve_key(Contractd,&keys[0])),"ipc/storage_unavailable");
    i.journal_mut().faults.unavailable=false;i.refresh(1001).unwrap();
    for k in &keys { assert_eq!(i.resolve_key(Contractd,k).unwrap().invalidated,Some(invalidation_reason_t::compromised)); }
    i.journal_mut().faults.fail_appends=false;
    for (n,k) in keys.iter().enumerate() {
        let weaker=builders::invalidation(&u.identity,k,invalidation_reason_t::rotated,&format!("r{n}"));
        i.register_invalidation(Vaultd,1001,&weaker).unwrap();
        assert_eq!(i.resolve_key(Contractd,k).unwrap().invalidated,Some(invalidation_reason_t::compromised));
    }
    for o in compromise.iter().rev() { i.register_invalidation(Vaultd,1001,o).unwrap(); }
    let mut restarted=identity_service_t::start(i.journal().clone()).unwrap();
    for k in &keys { assert_eq!(restarted.resolve_key(Contractd,k).unwrap().invalidated,Some(invalidation_reason_t::compromised)); }
    // A pending rotation is covered by a later durable compromise, including retry.
    let (mut i,u)=enrolled_identity();
    let rotated=builders::invalidation(&u.identity,&u.persistent.key_id,invalidation_reason_t::rotated,"pending rotated");
    i.journal_mut().faults.fail_appends=true;
    assert_eq!(code(i.register_invalidation(Vaultd,1001,&rotated)),"ipc/restriction_not_durable");
    i.journal_mut().faults.fail_appends=false;
    let c=builders::invalidation(&u.identity,&u.persistent.key_id,invalidation_reason_t::compromised,"later c");
    i.register_invalidation(Vaultd,1001,&c).unwrap();i.register_invalidation(Vaultd,1001,&rotated).unwrap();
    assert_eq!(i.resolve_key(Contractd,&u.persistent.key_id).unwrap().invalidated,Some(invalidation_reason_t::compromised));
}

#[test] fn r2_identity_restart_limit_vault_bundle_blocks_then_retries() {
    let (root,i,mut v,k)=setup("r2_restart_bundle");
    i.borrow_mut().journal_mut().faults.fail_appends=true;
    assert_eq!(code(v.invalidate_key(Alice,&k.persistent_key_id,invalidation_reason_t::compromised,&passphrase(Pass))),"ipc/restriction_not_durable");
    assert!(root.join("1001/pending.reg").exists());
    let journal=i.borrow().journal().clone();
    *i.borrow_mut()=identity_service_t::start(journal).unwrap();
    // Explicit staged limit: memory denial is absent after identityd alone restarts.
    assert_eq!(i.borrow_mut().resolve_key(Contractd,&k.persistent_key_id).unwrap().invalidated,None);
    assert_eq!(code(v.register_proposal(Contractd,request("still blocked",&payload(&k)))),"ipc/restriction_not_durable");
    i.borrow_mut().journal_mut().faults.fail_appends=false;
    assert_eq!(code(v.register_proposal(Contractd,request("durable",&payload(&k)))),"key_authority/key_invalidated");
    assert_eq!(i.borrow_mut().resolve_key(Contractd,&k.persistent_key_id).unwrap().invalidated,Some(invalidation_reason_t::compromised));
    assert!(!root.join("1001/pending.reg").exists());
}

#[test] fn r2_window_boundaries_and_unreadable_times() {
    let (mut i,u)=enrolled_identity();let mut r=i.resolve_key(Contractd,&u.temporary.key_id).unwrap();
    let start=StartUtc;let end=StartUtc+86400;
    for (now,ok) in [(i64::MIN,false),(start-1,false),(start,true),(end,true),(end+1,false),(i64::MAX,false)] {
        assert_eq!(check_signing_window(&r,now).is_ok(),ok,"{now}");
    }
    r.not_before="bad".into();assert_eq!(code(check_signing_window(&r,start)),"key_authority/certificate_window");
    r.not_before=builders::T0.into();r.not_after=Some("2026-02-30T08:00:00Z".into());
    assert_eq!(code(check_signing_window(&r,start)),"key_authority/certificate_window");
    r.not_after=None;assert!(check_signing_window(&r,i64::MAX).is_ok());
    let (_,_,mut v,k)=setup("r2_window_start");
    let p=commit_payload("commit_temporary_session",1001,&k.user_id,&k.temporary_key_id,authority_class_t::temporary,builders::T0);
    let a=v.register_proposal(Contractd,request("at start",&p)).unwrap();
    v.approve_and_sign(Alice,&a.proposal_id,&a.approval_digest,&passphrase(Pass)).unwrap();
}

struct r2_action_clock_t { armed:Rc<Cell<u32>>, utc:Rc<Cell<i64>>, action:RefCell<Option<Box<dyn FnOnce()>>> }
impl vault_clock_t for r2_action_clock_t {
    fn monotonic_seconds(&self)->u64 { let n=self.armed.get();if n>0 {self.armed.set(n-1);if n==1 {if let Some(f)=self.action.borrow_mut().take(){f();}}}1000 }
    fn utc_seconds(&self)->i64 {self.utc.get()}
}
fn r2_scheduled(label:&str)->(std::path::PathBuf,identity_t,vault_service_t<in_process_identity_t<memory_journal_t>,fixture_registry_t,r2_action_clock_t>,created_keys_t,Rc<Cell<u32>>,Rc<Cell<i64>>) {
    let root=temp_root(label);let i=identity_service();let armed=Rc::new(Cell::new(0));let utc=Rc::new(Cell::new(StartUtc));
    let mut v=vault_service_t::start(config(&root),in_process_identity_t{service:i.clone()},registry(&i),r2_action_clock_t{armed:armed.clone(),utc:utc.clone(),action:RefCell::new(None)}).unwrap();
    let k=v.create_user_keys(Alice,&passphrase(Pass)).unwrap();(root,i,v,k,armed,utc)
}
#[test] fn r2_window_changes_after_unlock() {
    let (_,_,mut v,k,arm,utc)=r2_scheduled("r2_kdf_window");
    utc.set(StartUtc+7*86400-5);
    let p=commit_payload("commit_temporary_session",1001,&k.user_id,&k.temporary_key_id,authority_class_t::temporary,"2026-10-20T07:59:55Z");
    let a=v.register_proposal(Contractd,request("r2_kdf",&p)).unwrap();
    let clock=utc.clone();v.clock_mut().action=RefCell::new(Some(Box::new(move||clock.set(StartUtc+7*86400+1))));arm.set(2);
    assert_eq!(code(v.approve_and_sign(Alice,&a.proposal_id,&a.approval_digest,&passphrase(Pass))),"key_authority/certificate_window");
    assert_eq!(arm.get(),0);
}
#[test] fn r2_final_check_refuses_missing_object_and_unavailable_authority() {
    for missing in [true,false] {
        let (root,i,mut v,k,arm,_)=r2_scheduled(if missing {"r2_missing"}else{"r2_unavailable"});
        let a=v.register_proposal(Contractd,request("r2_final",&payload(&k))).unwrap();
        let action:Box<dyn FnOnce()>=if missing {
            let path=root.join("1001").join(format!("{}.key",fixture::support::hex_encode(&k.persistent_key_id)));
            Box::new(move||std::fs::remove_file(path).unwrap())
        } else {Box::new(move||i.borrow_mut().journal_mut().faults.unavailable=true)};
        v.clock_mut().action=RefCell::new(Some(action));arm.set(2);
        assert_eq!(code(v.approve_and_sign(Alice,&a.proposal_id,&a.approval_digest,&passphrase(Pass))),if missing {"key_authority/key_not_certified"}else{"ipc/storage_unavailable"});
        assert_eq!(arm.get(),0);
    }
}
#[test] fn r2_rotation_and_invalidation_final_identity_check() {
    for rotate in [true,false] {
        let root=temp_root(if rotate {"r2_rot"}else{"r2_inv"});let i=identity_service();let mut v=interleaved_vault(&root,&i);
        let k=v.create_user_keys(Alice,&passphrase(Pass)).unwrap();let before=i.borrow().journal().record_count(1001);let shared=i.clone();
        v.identity_mut().arm(if rotate {4}else{2},move||push_record(&shared,1001,user_record_t::deletion_pending));
        let outcome=if rotate {v.rotate_key(Alice,authority_class_t::persistent,&passphrase(Pass)).map(|_|())}else{v.invalidate_key(Alice,&k.persistent_key_id,invalidation_reason_t::compromised,&passphrase(Pass))};
        assert_eq!(code(outcome),"key_authority/user_deleted");assert!(v.identity_mut().change.is_none());
        assert_eq!(i.borrow().journal().record_count(1001),before+1);
        assert_eq!(v.store().list_keys(1001).unwrap().len(),3);assert!(!root.join("1001/pending.reg").exists());
    }
}
#[test] fn r2_proposal_cancellation_expiry_and_restart_limits() {
    let (root,i,mut v,k)=setup("r2_proposal_edges");let p=payload(&k);
    let a=v.register_proposal(Contractd,request("cancelled",&p)).unwrap();
    assert_eq!(code(v.cancel_proposal(Bob,&a.proposal_id)),"ipc/owner_mismatch");
    v.cancel_proposal(Alice,&a.proposal_id).unwrap();
    assert_eq!(code(v.register_proposal(Contractd,request("cancelled",&p))),"ipc/duplicate_identifier");
    v.clock_mut().utc+=120;v.clock_mut().monotonic=900; // wall alone expires it
    let a=v.register_proposal(Contractd,request("cancelled",&p)).unwrap();
    v.approve_and_sign(Alice,&a.proposal_id,&a.approval_digest,&passphrase(Pass)).unwrap();
    assert_eq!(code(v.cancel_proposal(Alice,&a.proposal_id)),"ipc/proposal_used");
    v.clock_mut().monotonic+=120;v.clock_mut().utc-=120; // monotonic alone expires it
    let a=v.register_proposal(Contractd,request("cancelled",&p)).unwrap();
    drop(v);let mut v=vault(&root,&i);
    assert_eq!(code(v.approve_and_sign(Alice,&a.proposal_id,&a.approval_digest,&passphrase(Pass))),"ipc/proposal_unknown");
    v.register_proposal(Contractd,request("cancelled",&p)).unwrap(); // documented in-memory restart limit
}
#[test] fn r2_retired_per_user_quota_reserves_pending_transitions() {
    let (_,_,mut v,k)=setup("r2_quota_user");let p=payload(&k);let mut kept=0;
    for n in 0..31 {let a=v.register_proposal(Contractd,request(&format!("retired {n}"),&p)).unwrap();v.cancel_proposal(Alice,&a.proposal_id).unwrap();kept+=1;}
    let mut pending=Vec::new();for n in 0..8 {if let Ok(a)=v.register_proposal(Contractd,request(&format!("pending {n}"),&p)){pending.push(a);}}
    for a in pending {if v.cancel_proposal(Alice,&a.proposal_id).is_ok(){kept+=1;}}
    println!("per_user_retired_ids {kept}, limit 32, all original lifetimes live");
    assert!(kept<=32,"pending transitions exceed the stated retired-id bound: {kept} > 32");
}
#[test] fn r2_retired_global_quota_reserves_pending_transitions() {
    let root=temp_root("r2_quota_global");let i=identity_service();let mut v=vault(&root,&i);let mut users=Vec::new();let mut kept=0;
    for uid in 1001..=1009 {
        let k=v.create_user_keys(caller_role_t::trusted_client{uid},&passphrase(Pass)).unwrap();
        let p=commit_payload("commit_genesis",uid,&k.user_id,&k.persistent_key_id,authority_class_t::persistent,At);
        for n in 0..if uid==1009 {7}else{31} {
            let mut req=request(&format!("retired {uid} {n}"),&p);req.owner_uid=uid;
            let a=v.register_proposal(Contractd,req).unwrap();v.cancel_proposal(Contractd,&a.proposal_id).unwrap();kept+=1;
        }users.push((uid,p));
    }
    assert_eq!(kept,255);let mut pending=Vec::new();
    for (uid,p) in users.iter().take(8) {for n in 0..8 {
        let mut req=request(&format!("pending {uid} {n}"),p);req.owner_uid=*uid;
        if let Ok(a)=v.register_proposal(Contractd,req){pending.push(a);}
    }}
    for a in pending {if v.cancel_proposal(Contractd,&a.proposal_id).is_ok(){kept+=1;}}
    println!("global_retired_ids {kept}, limit 256, all original lifetimes live");
    assert!(kept<=256,"pending transitions exceed the stated global retired-id bound: {kept} > 256");
}

struct r2_failing_certificate_t {link:in_process_identity_t<memory_journal_t>,fail:bool}
impl identity_client_t for r2_failing_certificate_t {
    fn register_enrollment(&mut self,u:u32,e:&[u8],p:&[u8],t:&[u8])->service_result_t<enrollment_receipt_t>{self.link.register_enrollment(u,e,p,t)}
    fn register_certificate(&mut self,u:u32,c:&[u8])->service_result_t<append_receipt_t>{if self.fail {Err(service_error_t::ipc(ipc_code_t::role_denied))}else{self.link.register_certificate(u,c)}}
    fn register_invalidation(&mut self,u:u32,o:&[u8])->service_result_t<append_receipt_t>{self.link.register_invalidation(u,o)}
    fn resolve_key(&mut self,k:&digest_t)->service_result_t<key_resolution_t>{self.link.resolve_key(k)}
    fn user_identity(&mut self,u:u32)->service_result_t<user_identity_t>{self.link.user_identity(u)}
}
#[test] fn r2_definitive_registration_error_keeps_bundle_then_retries() {
    let root=temp_root("r2_regerr");let i=identity_service();
    let mut v=vault_service_t::start(config(&root),r2_failing_certificate_t{link:in_process_identity_t{service:i.clone()},fail:false},registry(&i),test_clock_t{monotonic:1000,utc:StartUtc}).unwrap();
    v.create_user_keys(Alice,&passphrase(Pass)).unwrap();let id=[21;16];open(&i,id,2);
    v.identity_mut().fail=true;v.registry_mut().status_unavailable=true;
    assert_eq!(code(v.recover_keys(Alice,&id,&passphrase(Pass))),"ipc/role_denied");
    let held=v.store().list_keys(1001).unwrap();assert_eq!(held.len(),5);assert!(root.join("1001/pending.reg").exists());
    v.registry_mut().status_unavailable=false;
    assert_eq!(code(v.recover_keys(Alice,&id,&passphrase("wrong"))),"ipc/role_denied");assert_eq!(v.store().list_keys(1001).unwrap(),held);
    v.identity_mut().fail=false;let named=v.recover_keys(Alice,&id,&passphrase("wrong")).unwrap();
    assert!(held.contains(&named.0)&&held.contains(&named.1));assert_eq!(v.store().list_keys(1001).unwrap().len(),3);
}
#[test] fn r2_definitive_registration_error_abort_erases_only_new_keys() {
    let root=temp_root("r2_regabort");let i=identity_service();
    let mut v=vault_service_t::start(config(&root),r2_failing_certificate_t{link:in_process_identity_t{service:i.clone()},fail:false},registry(&i),test_clock_t{monotonic:1000,utc:StartUtc}).unwrap();
    v.create_user_keys(Alice,&passphrase(Pass)).unwrap();let original=v.store().list_keys(1001).unwrap();let id=[22;16];open(&i,id,2);
    v.identity_mut().fail=true;assert_eq!(code(v.recover_keys(Alice,&id,&passphrase(Pass))),"ipc/role_denied");assert_eq!(v.store().list_keys(1001).unwrap().len(),5);
    push_record(&i,1001,user_record_t::containment);
    assert_eq!(code(v.recover_keys(Alice,&id,&passphrase(Pass))),"admission/recovery_rule");assert_eq!(v.store().list_keys(1001).unwrap(),original);assert!(!root.join("1001/pending.reg").exists());
}
#[test] fn r2_enrollment_retry_requires_correct_passphrase() {
    let root=temp_root("r2_enrollretry");let i=identity_service();let mut v=vault(&root,&i);
    i.borrow_mut().journal_mut().faults.fail_appends=true;
    assert_eq!(code(v.create_user_keys(Alice,&passphrase(Pass))),"ipc/storage_unavailable");let held=v.store().list_keys(1001).unwrap();assert_eq!(held.len(),3);
    i.borrow_mut().journal_mut().faults.fail_appends=false;
    assert_eq!(code(v.create_user_keys(Alice,&passphrase("wrong"))),"ipc/authentication_failed");assert_eq!(i.borrow().journal().record_count(1001),0);
    let k=v.create_user_keys(Alice,&passphrase(Pass)).unwrap();assert!(held.contains(&k.user_id));assert_eq!(i.borrow().journal().record_count(1001),3);
}
```

## Evidence integrity and next action

Local evidence is retained outside the publication. Qualification output `out/m2/qualification/amd64_20261013T115544Z_1/`; unit `out/m2/runs/unit_20261013T115659Z_1767/`; harness `out/m2/runs/harness_20261013T120108Z_3790/`; positive protection `out/m2/protection/vault_protection_report.txt`. The candidate remains detached at ca40c62; final `git status --porcelain` is empty and `git diff --exit-code` exits 0. Only the Markdown review is committed.

| Evidence | SHA-256 |
|---|---|
| docker-build.log | `87678f371c8db1dd6c310287863de4e7ff3316df619c5d201b2df4b3c8b2dd8a` |
| qualification.log | `cbf313ee1ba3c368d164a84b43d59ca090688f0c129dfd98609f92465a2674ea` |
| protection-positive.log | `c55dd11edee363c573111c85596b9bf178cc88487ec0302dab2f706603d98dd7` |
| static-checks.log | `4de7404dff863ec97ff5b62d03e3459cd696c6f71478443d6d00749f021f0bfb` |
| static-tests.log | `647a7968a0c15e4acee6bee185e1e2566dd25e0087aa4ce49c8f6dcc0c592f13` |
| prior-debug-locked.log | `bcaa600957b5760b161227db05b33c5c33f08688fac097c1336b4d5741af2ae8` |
| prior-release.log | `71d72c7be6752f9a7592a4da8b00d320dd3ffc6542363fea8648e66407834ebc` |
| new-debug.log | `4e44f729c8022e8c2fc4c868169aa35f87e72b0d846250f9622b83d8a9a12b04` |
| new-release.log | `c2da7266d8a9ebe1c03279f72b6cc19137584b0f4ff35527fda91dd6995d9960` |
| journal-scenarios.log | `fbcb3e733095dd73a6565ecb1f017a577191532e0e76f5dfef705246387597b5` |
| secret-custody-grep.log | `9c0faf93d5a47841e20f8282c51d7e812890d9435750d660333491a32b714df5` |
| ci-log-115131427974.txt | `bec90186b1e86e37bde83536752deb4911d792a60171bb7fc1182cc59590277d` |
| unit cargo_test_debug.json | `dad963024d77cefe985c9c1a505aca40f8c987b4a13909c1a50bb67f1a0fa03f` |
| unit cargo_test_release.json | `8abcc7c7086c0104eec070dfc1d569657f07f19665d20b013f7869844a786d0a` |
| unit format_differential.json | `101c53fbef05dbcebf4f1a1a6990852c107fdc64304931b7f1d2216016812300` |
| harness python_harness.json | `95e48a0303773f4d7c5aa08760ff4448871b445c9f0f88c59e01dcea09ce606a` |

Return M2-004-R2-01 for implementation and exact-commit independent re-review. Preserve the three staged obligations and the final M2-008 plugin requirement. No architecture, risk acceptance, integration or milestone-completion decision is made for the owner.
