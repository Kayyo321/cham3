# Handoff: M2-001 / M2-002 revision 2 — request for independent re-review

- **State:** M2-001 and M2-002 revision 2 **IN_REVIEW**. Every G1–G6 proposal is PROPOSED (revision 2); none is approved. M2-003..M2-008 remain DRAFT. Full M2 is not DONE.
- **Role:** IMPLEMENTER (and ARCHITECT for the proposals). The author does not approve this work.
- **Model/provider:** Claude Code (Opus 5.5). Requested reviewer: Codex (independent).
- **Working directory / branch:** /Users/jessicabruce/cham3, `codex/m2-cryptographic-contract-core`.
- **Task specification:** [M2 task](../tasks/M2-cryptographic-contract-core.md) (M2-001, M2-002, G1–G6); [Claude prompt](../prompts/m2_claude_implementation.md).
- **Review answered:** `docs/reviews/M2-001-002-codex-security-review.md` at 9cf882503848f8c6cc4938a588cc9fc062a9392d (on `main`, PR #1; not merged into this branch), CHANGES_REQUESTED for M2-001 and G1–G6. Author response: [finding resolutions](../reviews/M2-001-002-finding-resolutions.md).
- **Baseline for this revision:** 3c35aac0e8d93fef334fc4f208e1365857b1e84a (revision-1 evidence/handoff, clean).
- **Revision-2 implementation commit (exact re-review candidate for code, specs, ADRs, vectors and locks):** a2bbb370c867da57fdc72274fb17ca19a23426e4. The commit that adds this handoff and the revision-2 evidence directories changes documentation/evidence only.

Product authority: PROJECT.md and approved ADR 0001.

## Completed work

| Finding | Status | Where |
|---|---|---|
| R01 identity false positive | implemented: `build/scripts/m2_source_identity.sh`; validated environment collector; `source_identity` step in dispatcher and qualification; 8 canaries | `tests/m2/harness/test_m2_source_identity.py` |
| R02 amd64 locks | evidence: reviewer-reproduced locks committed byte-identically under `build/m2/debian_lock_amd64.txt` / `debian_debs_amd64.sha256` | ADR 0003 §2 |
| R03 Argon2 workspace | implemented in the probe (`argon2_workspace_t`, 4 tests incl. library regression control); vault ownership, budget and client custody specified | ADR 0003 §7, ADR 0005 §4–§7 |
| R04 restriction loss after restart | design revised: run marker, restriction ledger, containment; residual withdrawn | ADR 0006 §3, §9.2; ADR 0007 §2 |
| R05 identity/key freshness | design revised: identity state in the anchored journal; stateless identityd; coordinated restore | ADR 0006 §1, §5, §8; ADR 0005; ADR 0008 §5 |
| R06 identity rebinding | design + oracle/vectors: authoritative subject, explicit `identity_change` | spec §6.9, ADR 0004 §7a |
| R07 network authority | design + oracle/vectors: network as grants; restore scope incl. network/expiration | spec §6.10, §6.7; ADR 0004 §7b; ADR 0007 §1 |
| R08 size contract | design + oracle/vectors: 61 440-byte objects; 4 096-byte wrapper budget; pagination; boundary vectors | spec §2.3; IPC §2 |
| R09 clock rollback | design revised: monotonic BOOTTIME high-water, floor records, expiry latch, new-boot rules | ADR 0007 §5 |
| R10 anchor serialization | design revised: single in-memory map, global lock, lock order, durable-slot acknowledgment | ADR 0006 §2 |
| R11 deletion | design revised: tombstone-first ceremony | ADR 0006 §7, ADR 0008 §4 |
| I01, I02, I03 | wording corrected; secure-memory valgrind in qualification; scope section | ADR 0006 §2; qualify script; ADR 0005 §8 |

Also: `build/scripts/m2_advisory_review.py` and author-run advisory evidence (reviewer's own check was BLOCKED); `m2_checks.yml` requires a clean tree and uploads text evidence only.

## Verification

| Check | Environment / command | Result | Evidence / limits |
|---|---|---|---|
| Full qualification at clean `a2bbb37` | arm64 Debian 13.7 container, UID 1000, `CHAM3_M2_REQUIRE_CLEAN=1 bash build/scripts/qualify_m2_rust.sh` | PASS (development) | [m2_001_arm64_a2bbb37](../evidence/M2-cryptographic-contract-core/m2_001_arm64_a2bbb37/): `source_identity` PASS (clean), arm64 locks, sources, inventory, rustfmt, clippy, naming, profiles, builds, backends, valgrind probe and secure-memory tests (10 passed, 0 errors), unit (36 + 36 cargo tests, provider oracle, ECR oracle 18/90/14), harness (70 tests). Not target qualification |
| Full qualification at `a2bbb37` on amd64 | GitHub Actions `m2_checks` [run 38283264875](https://github.com/Kayyo321/cham3/actions/runs/38283264875) (GitHub-hosted amd64 runner, kernel 6.17.0-1022-azure, pinned Debian 13.7 container, UID 1001, `CHAM3_M2_REQUIRE_CLEAN=1`) | **PASS** (every check) | [m2_001_amd64_ci_a2bbb37](../evidence/M2-cryptographic-contract-core/m2_001_amd64_ci_a2bbb37/): clean `source_identity`; `installed_packages_lock` and `downloaded_debs_lock` against the committed amd64 locks with empty diffs; sources, inventory, lint, builds, backends, valgrind probe and secure-memory tests (10 passed, 0 errors); unit 36 + 36, ECR oracle 18/90/14; harness 70. Artifact 11762796075 (84 281 bytes, `sha256:6e10bd1f…bb56`). Author-retrieved from CI; independent rerun requested |
| M0 / M1 regression | GitHub Actions `m0_checks` [38283264828](https://github.com/Kayyo321/cham3/actions/runs/38283264828), `m1_checks` [38283264829](https://github.com/Kayyo321/cham3/actions/runs/38283264829) at `a2bbb37` | PASS / PASS | Workflow logs |
| M0/M1 static gates | arm64 container: `build/scripts/run_static_checks.sh`; `tests/run_tests.sh static` | PASS | Staged revision-2 tree before commit; 82 static tests |
| Reviewer tools as regression controls | arm64 container: reviewer `schema_holes.py` and `independent_vectors.py` from the review archive against revision-2 oracle/vectors | `schema_holes.py` exits 1 at its first case (`subject_binding_mismatch`, no longer ACCEPTED); `independent_vectors.py` PASS | Each of the four holes replayed individually in `test_m2_review_regressions.py` (PASS) |
| Advisory review | arm64 container, `build/scripts/m2_advisory_review.py` on Debian tracker snapshot `52d2faea…5dc6` and advisory-db `25a288f6…2569` | PASS (0 behind, 0 affected crates, 0 open issues in rustc/rust-* sources); 23 untriaged open items outside the Rust graph reported | [advisory_review_20261012](../evidence/M2-cryptographic-contract-core/advisory_review_20261012/advisory_readme.md); author-run, needs reviewer repetition |
| CI artifact lock bytes vs reviewer proposals | author: `gh run download 38276640049 -n m2_qualification_amd64` | Identical SHA-256 digests | Author evidence only; the reviewer did not compare them |
| Design mechanisms R04, R05, R09, R10, R11 | — | NOT_RUN | Gated implementation (M2-004..M2-006); fault cases enumerated in the ADRs |
| Miri / sanitizers / coverage-guided fuzzing | — | NOT_AVAILABLE | Pinned toolchain limit; never PASS |
| M2 security behavior A03–A15 | — | NOT_RUN | No domain logic exists |

## Re-review request

Please review `3c35aac..a2bbb37` (and the documentation-only evidence/handoff commit after it) against the original findings, PROJECT.md, ADR 0001 and the task, and return **APPROVE** or **CHANGES_REQUESTED** per package (M2-001) and per gate (G1–G6). The original review record is unchanged on `main`.

amd64 qualification (disposable, unprivileged; the amd64 locks are now committed, so a full PASS is expected):

~~~sh
git fetch origin codex/m2-cryptographic-contract-core
git checkout --detach a2bbb370c867da57fdc72274fb17ca19a23426e4
docker build -f build/m2/dev_container.dockerfile -t cham3-m2-dev build/m2
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
  -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
  -v "$PWD:/work" -w /work cham3-m2-dev bash build/scripts/qualify_m2_rust.sh
~~~

Suggested focus:

- R01: repeat the no-`.git` export reproduction (`python3 -I tests/m2/lib/m2_harness.py environment --run-dir …` must now exit 1) and `build/scripts/m2_source_identity.sh` on an export.
- R03: inspect `argon2_workspace_t` and its tests; confirm the vault design names owners for every secret buffer.
- R04/R05/R10/R11: look for unclean-run, cross-domain restore or concurrency paths the revised tables miss; check that containment never resumes access from missing evidence and that the ledger/run-marker failure cases are complete.
- R06–R08: re-run your `independent_vectors.py`/`schema_holes.py` or new probes; try to find another identity, network, restore or size hole in the revised spec/vectors.
- R09: try idle-period, suspend and restart rollback cases against ADR 0007 §5.
- Repeat the advisory review with your own access (`m2_advisory_review.py` takes the tracker JSON and advisory-db checkout as inputs).

## Owner decisions required after re-review (not taken)

1. Approve, amend or reject each of G1–G6 (ADRs 0003–0008, revision 2).
2. ADR 0006 §9.1 default anchor mode (recommended: degraded `same_volume` with visible status).
3. ADR 0006 §9.2 lifting containment after an unverifiable unclean run (recommended C1: fresh signed per-lineage re-authorization; quarantine additionally needs the admin ceremony).
4. ADR 0007 §4 one-shot semantics (recommended: reservation consumes).
5. ADR 0008 §3 deletion granularity (recommended: whole-account only).

## Remaining

- **Known risks/limits:** containment after reboots following an unclean run costs users reconfirmation; degraded rollback protection by default; passphrase approval through a terminal is not trusted input/display or human-presence proof; identities and consumers are fixtures until M3; cross-boot clock rollback by root is a stated limit; open untriaged glibc/systemd items must be rechecked before packaging (M2-007); Debian updates require re-qualification.
- **Uncommitted paths or dirty state:** none after the evidence/handoff commit (`out/` is ignored).
- **Required next action and owner:** Codex re-reviews the exact commits → implementer repairs any findings → owner decides G1–G6 and the four explicit choices → only then may M2-003 onward become READY.
