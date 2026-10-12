# M2-001 / M2-002 independent security re-review, revision 5

Reviewer: **Codex**, independent REVIEWER, 2026-10-12. No implementation authorship, implementation edits, merges, delegation, or owner decisions. Only this review record is published.

Candidate: **28a8595e627bcd092acaff4b8161522cf5d38f7f**, tree **ad08a16302ea51d8b9f9322ca6599b50c987f2ea**, range **70c431c13a1c76fd2948e6c8f0ae33b720618043..28a8595e627bcd092acaff4b8161522cf5d38f7f**. The prior reviewed implementation is **9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a**. Follow-ups **ed2ba0d79e60b9a476af5b93685a1b4b214573f2** and **87c9cbc9bdeed40fd90b93213c22e5b854502b29** change only `docs/` (1 and 36 paths respectively). Their parent chain is candidate → ed2ba0d → 87c9cbc.

Review branch: `review/m2-001-002-r5-security`, based on main **9cf882503848f8c6cc4938a588cc9fc062a9392d**. The existing main checkout was clean and remains untouched. An independent, UID-1001-owned clone is detached at the candidate. The outdated `~/Cham3` location was not used; it does not exist in this shell.

Read AGENTS.md, PROJECT.md (including trust boundaries, cryptography, signing/authorization/enforcement separation and ECR requirements), WORKFLOW, ORCHESTRATION, ROLES, START_PROMPTS, TESTING, SECURITY_INVARIANTS, STATUS, M2 task/gates, review template, ADRs 0003–0008 and relevant oracle/interface code. Read the r4 independent review from **3a3b5d17ddc44b96c3fcc8265c5b337066aff0fb**, the author finding resolutions r4 and handoff r5 from **87c9cbc**, and the exact candidate/follow-up diffs. The r4 review independently hashes to **4d1d22b89e06460d73a2d9e60aa39ef924f83ac02fc5eafce2fdeaae9f8d433d**. Author records and CI labels were treated as claims to verify.

## Verdicts and scope

| Item | Verdict | Independent basis |
|---|---|---|
| M2-001 | **APPROVE** | Fresh native amd64 pinned qualification passes; own generated inventory and own advisory retrieval/review pass. |
| G1 / ADR 0003 | **APPROVE for owner consideration** | Independent Debian prerequisite is satisfied; zero behind resolved fixes, zero affected RustSec versions; all 23 non-unimportant open issues judged for the current scope below. |
| G2 / ADR 0004 | **APPROVE for owner consideration** | Normative text unchanged; independent canonical bytes/signatures/hashes, oracle check and exact regeneration pass. |
| G3 / ADR 0005 | **APPROVE for owner consideration** | Unchanged authenticated vaultd-only naming route, closed arguments and reconciliation; R14 controls pass. |
| G4 / ADR 0006 | **APPROVE for owner consideration** | Unchanged design/pending-tail implementation; I04 observation and sensitivity repaired and independently verified. |
| G5 / ADR 0007 | **APPROVE for owner consideration** | Unchanged honest R15 limits and explicit unselected options; time model/regressions pass. |
| G6 / ADR 0008 | **APPROVE for owner consideration** | Unchanged coordinated recovery, persistent-only restore, privacy/quota and tombstone rules. |

These are independent recommendations for the exact qualification/design package. They make **no owner decision**, do not make M2-002/full M2 DONE, and do not admit dependent implementation without its owner/dependency gates. No actual vault, registry, IPC socket, durable anchor, service clock, resource enforcement, KDE authorization or packaged setgid client is accepted by this review. There are **no new confirmed scoped findings**.

## Environment and reproducible commands

Host: **a03-hating-machine**, Debian **13.7 trixie**, Linux **7.1.13+deb13-amd64**, x86_64; reviewer and container **UID/GID 1001:1001**. Docker client/engine **26.1.5+dfsg1**, containerd **1.7.24~ds1-6+deb13u1**, runc **1.1.15+ds1-2+b4**, overlay2, AppArmor, builtin seccomp and cgroup namespace. Pinned image is Debian 13.7 amd64, ID **sha256:4f4f684d76f7c3c299932846ebe66765204c7e15c9414d77922d15ff561bb327**. Native execution, no emulation, privileged containers, added capabilities, production secrets or `--record`.

Docker was initially absent. The owner explicitly authorized installing all prerequisites during this review. `sudo -n apt-get update` and `sudo -n apt-get install -y docker.io shellcheck yamllint python3-cbor2 python3-nacl` succeeded (23 new packages including dependencies, no upgrades/removals). Standard Docker/containerd services started. No Docker-group membership was added and no host protection was disabled. `sudo` accesses the daemon; `--user` sets the actual qualification/test process credentials to 1001.

HTTPS Debian tracker, GitHub and RustSec retrievals succeeded with normal TLS. No proxy variables were set. Host DNS uses loopback `127.0.2.1`; default Docker DNS selected 8.8.8.8/8.8.4.4 and the initial build retried archive lookups. That build was cancelled. **Build only** was rerun with `--network=host`, using the host DNS. The Dockerfile, authenticated package inputs and locks were unchanged. Qualification/advisory containers use default container networking.

Docker's initial default descriptor limit exceeded one billion, preventing both Valgrind executables from starting. The first completed qualification therefore correctly **FAILed**. The final fresh run adds **`--ulimit nofile=65536:65536`**, a lower resource limit, with all security/test checks retained. The advisory invocation uses the same setting. No source change or check bypass was made.

Evidence aliases (local raw evidence, not additional published files):

- **S**: `/home/dev/cham3-review-r5-candidate` (clean clone).
- **E**: `/home/dev/cham3-review-r5-evidence`.
- **Q**: S/`out/m2/qualification/amd64_20261012T221102Z_1` (final PASS).
- **U**: S/`out/m2/runs/unit_20261012T221121Z_1599`.
- **H**: S/`out/m2/runs/harness_20261012T221130Z_2675`.
- **Q0**: S/`out/m2/qualification/amd64_20261012T220957Z_1` (descriptor-limit FAIL).

Exact setup and final container aliases:

```sh
git fetch origin
git clone --no-hardlinks /home/dev/cham3 /home/dev/cham3-review-r5-candidate
git -C /home/dev/cham3-review-r5-candidate checkout --detach 28a8595e627bcd092acaff4b8161522cf5d38f7f
stat -c '%u:%g %n' /home/dev/cham3-review-r5-candidate
cd /home/dev/cham3-review-r5-candidate
sudo -n docker build --network=host -f build/m2/dev_container.dockerfile -t cham3-m2-r5-review build/m2
run() {
  sudo -n docker run --rm --user "$(id -u):$(id -g)" --ulimit nofile=65536:65536 \
    -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
    -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
    -v /home/dev/cham3-review-r5-candidate:/work -w /work cham3-m2-r5-review "$@"
}
review_run() {
  sudo -n docker run --rm --user "$(id -u):$(id -g)" --ulimit nofile=65536:65536 \
    -e HOME=/tmp/cham3_home -e CHAM3_M2_REQUIRE_CLEAN=1 \
    -e GIT_CONFIG_COUNT=1 -e GIT_CONFIG_KEY_0=safe.directory -e GIT_CONFIG_VALUE_0=/work \
    -v /home/dev/cham3-review-r5-candidate:/work:ro \
    -v /home/dev/cham3-review-r5-candidate:/home/dev/cham3-review-r5-candidate:ro \
    -v /home/dev/cham3-review-r5-evidence:/review -w /work cham3-m2-r5-review "$@"
}
```

These aliases are saved as E/run_container.sh and E/review_container.sh, invoked with `bash`. The second read-only source mount lets external scripts use their native absolute source paths; it adds no authority.

## Command / evidence ledger

All paths below use the aliases above. Expected-negative probe exit 1 is evidence of rejection, not a candidate failure.

| Check | Status | Exact command / environment | Evidence and observation |
|---|---|---|---|
| Startup / exact candidate | PASS | Setup above; `git status --porcelain`; `git rev-parse HEAD`; `git diff --exit-code` in S | Empty tracked/untracked status; requested SHA/tree; clone owned 1001:1001; Q/U/H identity files say dirty_tracked 0, euid 1001. |
| r4 review integrity | PASS | `git show 3a3b5d1:docs/reviews/M2-001-002-codex-security-rereview-r4.md` into E/prior_review.md; `sha256sum` | Exact digest above. |
| Protected implementation paths | PASS | `git diff --exit-code 9b1aa2b 28a8595 -- core Cargo.toml Cargo.lock build .github PROJECT.md docs/decisions docs/specs tests/m2/run_m2_tests.sh tests/m2/lib` | E/unchanged_inputs.log empty, exit 0. |
| Earlier build/provider inputs | PASS | `git diff --exit-code 84fa6ef 28a8595 -- core Cargo.toml Cargo.lock build .github PROJECT.md AGENTS.md docs/decisions/0001_eternal_contract_repository.md docs/engineering tests/m2/run_m2_tests.sh tests/m2/lib` | E/prior_inputs_unchanged.log empty, exit 0. |
| Candidate patch / following docs | PASS | `git diff 70c431c 28a8595`; `git diff ed2ba0d^ ed2ba0d`; `git diff 87c9cbc^ 87c9cbc`; `git diff --check 70c431c 28a8595`; `python3 -I E/verify_delta.py` | E/candidate.diff, docs_ed2ba0d.diff, docs_87c9cbc.diff, verify_delta.log. Candidate has only five test/oracle files plus author response; follow-ups all docs/. Reviewed progress/evidence wording takes no owner decision. |
| Vectors exact comparison | PASS | `python3 -I E/verify_delta.py` | Removing exactly nine added `keys` objects yields the prior JSON object **and exact prior bytes**. Only the three recovery_naming_* scenarios change; 22 scenarios/346 steps unchanged. |
| Missing Docker / initial native checks | BLOCKED / FAIL, resolved | Initial `docker build ...` and `docker run ... bash build/scripts/qualify_m2_rust.sh`: exit 127. Native `bash build/scripts/run_static_checks.sh`; `sh tests/run_tests.sh static` | E/docker_build.log, qualification.log: docker not found. Native static checks FAIL missing shellcheck/yamllint; 82 static tests with one dependency failure. Native source-identity subset: 15 tests, two prerequisite failures (no cargo/rustc). These are preserved and superseded by qualified runs, not product findings. |
| Temporary native tool setup/checks | PASS, supplemental | `apt-get download shellcheck=0.10.0-1 yamllint=1.37.1-1 python3-cbor2=5.6.5-1 python3-pathspec`; later python3-nacl=1.5.0-7; `dpkg-deb -x`; venv with extracted Debian modules | E/tools_debs, tools_root, venv; no initial host package mutation. PATH=E/venv/bin:$PATH native static checks and 82 tests then PASS. Final acceptance uses pinned container results. |
| Owner-authorized prerequisites | PASS | `sudo -n true`; `sudo -n apt-get update`; `sudo -n apt-get install -y docker.io shellcheck yamllint python3-cbor2 python3-nacl`; `sudo -n docker version`; `sudo -n docker info --format '{{json .SecurityOptions}} {{.Architecture}} {{.Driver}}'` | E/docker_version.txt, docker_settings.txt; versions and security settings above. |
| First default-network build | BLOCKED, cancelled | `sudo -n docker build -f build/m2/dev_container.dockerfile -t cham3-m2-r5-review build/m2` | E/docker_build_after_install.log: repeated InRelease lookup retries, then CANCELED/context canceled. No pin change. |
| Pinned image build | PASS | Build command above; `sudo -n docker image inspect cham3-m2-r5-review --format '{{.Id}} {{.Architecture}}'` | E/docker_build_host_network.log, exit 0; pinned image ID/amd64 above. |
| Initial qualification Q0 | FAIL, environmental prerequisite resolved | Same run settings **without** --ulimit; `bash build/scripts/qualify_m2_rust.sh` | E/qualification_after_install.log. Only both Valgrind checks FAIL: `FATAL: Private file creation failed`, reported descriptor limit 1073741804; all builds/suites pass. |
| Fresh final qualification | PASS | `run bash build/scripts/qualify_m2_rust.sh` | E/qualification_nofile_65536.log; Q/summary.txt, exit 0; all required checks PASS, no --record. |
| Static checks | PASS | `run bash build/scripts/run_static_checks.sh` (executed before lowering nofile) | E/static_checks_container.log, exit 0; shellcheck 24, Python compile 62, YAML 5, configured secret scan. |
| Static tests | PASS | `run sh tests/run_tests.sh static` (before lowering nofile) | E/static_tests_container.log; 82 tests, OK, exit 0. |
| Preserved early-effect probe | PASS, expected rejection | `review_run python3 -I /review/early_effect_probe_original.py` | E/early_effect_probe_container.log; unchanged code block extracted from r4, exits 1 on the restart scenario's three direct key-state failures. Probe stops at that assertion; retry coverage is separately established below. Original extraction digest 2cc05d5aa138905e5fa379b964395338e78e25d8b10b86e54879632edca98358. |
| Early-effect baseline | PASS, gap reproduced | E/venv/bin/python3 -I E/early_effect_probe_baseline.py | E/early_effect_baseline.log; old oracle/vectors from `git show 9b1aa2b:...`, only paths adapted; all three naming scenarios return [], exit 0. |
| I04 new harness sensitivity / own variants | PASS, model | `review_run python3 -I /review/i04_variant.py` (also native) | E/i04_variant_container.log; new shipped test passes; removing only `keys` observations in memory makes it raise AssertionError with no detected failures. Own epoch-only and retirement-only pre-anchor mutants produce 3/2 **key-state-only** failures in restart/retry scenarios, preserving outcomes, results and record counts. |
| R14 / earlier semantic regressions | PASS, model | Q invokes `sh tests/m2/run_m2_tests.sh harness`; native `python3 -I -m unittest discover -v -s tests/m2/harness -p test_m2_review_regressions.py` | H/python_harness.log/json: 120 inventory-exact tests; targeted native subset 26 PASS. Defect tests include original and response-preserving naming, role table, no direct route, retry/tail/reconciliation controls and earlier subject/network/restore controls. |
| R15 time limits/model | PASS, disclosed limits | `review_run python3 -I /review/time_limits_probe.py`; qualified harness; native unittest discover with -p test_m2_trusted_time.py | E/time_limits_probe_container.log, exit 0 reproduces the already-disclosed 5 s floor / 1735 s remaining counterexample and arbitrary forward acceleration. Printed historical FAIL labels describe old claims, not current wording. 20 targeted time tests PASS; service clock NOT_RUN. |
| R01-r2 collector canaries | PASS | `review_run python3 -I -m unittest discover -v -s /work/tests/m2/harness -p test_m2_source_identity.py`; qualified harness | E/source_identity_container.log: 15 tests PASS; failures remain fail closed. |
| Preserved independent tools | PASS | `git show 9cf8825:docs/reviews/M2-001-002-codex-security-review-evidence.tar.gz`; `tar -xzf`; `sha256sum -c SHA256SUMS` | E/prior_tools_integrity.log; archive digest 8655477da0c83b186e9468cf874e85fc4a6536d9e5401327c44ef1c9159ab5af. r2 probe extracted from 336f13b appendix, digest 8dcdde94144edb014a662ba0d0b69db356b095b02499272fbd68092daa16ee8f. |
| Independent signed vectors | PASS | `review_run python3 -I /review/cham3_m2_review_evidence/independent_vectors.py tests/m2/vectors/ecr_v1/vectors.json` | E/independent_vectors_container.log; 18 named-object canonical envelope/payload/header, signature/hash and changed-preimage controls. Historical final line hardcodes 17; the 18 per-object checks establish the actual coverage. |
| Original subject probe | PASS, expected rejection | Native E/venv/bin/python3 -I E/cham3_m2_review_evidence/schema_holes.py tests/m2/oracle/ecr_oracle.py tests/m2/vectors/ecr_v1/vectors.json | E/schema_holes.log; exits 1 at admission/subject_binding_mismatch. Later cases established by qualified regression tests. |
| Original r2 probe | PASS, expected rejection | `review_run python3 -I /review/r2_probes.py tests/m2/oracle/ecr_oracle.py tests/m2/vectors/ecr_v1/vectors.json` | E/r2_probes_container.log; exits 1 at admission/network_grant_conflict after earlier controls. Does not claim its unexecuted tail ran. |
| Oracle / exact regeneration | PASS | Q unit oracle check; `review_run sh -c 'python3 -I tests/m2/oracle/ecr_oracle.py generate /review/regenerated_vectors_container.json && cmp tests/m2/vectors/ecr_v1/vectors.json /review/regenerated_vectors_container.json && sha256sum tests/m2/vectors/ecr_v1/vectors.json /review/regenerated_vectors_container.json'` | U/ecr_vector_oracle.log and E/regeneration_container.log: 18/90/18/22/346; identical digest fb918ad6d486616d3b8d40be15c91734caaccc8ed09dc790c3d322333bf800b2. |
| Own Debian retrieval | PASS | `date -u +%Y-%m-%dT%H:%M:%SZ`; `curl --fail --location --max-time 60 --dump-header E/tracker_headers.txt --output E/tracker.json --write-out 'http=%{http_code} bytes=%{size_download}\n' https://security-tracker.debian.org/tracker/data/json`; `wc -c`; `sha256sum` | HTTP 200 at 2026-10-12T22:00:46Z; 82518506 bytes; SHA-256 3464ca13702eea97fc84042030476ec85efc6100aa2d7e4be20b83cc820e0848. Last-Modified 21:30:42Z. This is the reviewer's retrieval, not the author snapshot. |
| Own RustSec retrieval | PASS | `git clone https://github.com/RustSec/advisory-db.git E/advisory-db`; `git -C E/advisory-db rev-parse HEAD` | 57dcc0f2192e025138907a495c1ac9a7e1e3e123; all six matching nonwithdrawn advisory files read and ranges judged. |
| Own combined advisory run | PASS | `review_run python3 -I build/scripts/m2_advisory_review.py --tracker /review/tracker.json --advisory-db /review/advisory-db --inventory out/m2/qualification/amd64_20261012T221102Z_1/dependency_inventory.json --out-dir /review/advisory_own` | E/advisory_own.log and E/advisory_own/*; exit 0; own final qualification inventory, actual dpkg sources in same pinned image/settings. Also ran against Q0 inventory with identical results. |
| Independent source-map corroboration | PASS, supplemental only | `python3 -I E/supplemental_advisory.py`; per-package `apt-cache show <package>=<version>` at committed amd64 lock versions | E/apt_source_records.txt, independent_source_map.tsv, supplemental_advisory.log. Initially used CI inventory with explicitly substituted source map, **not** own qualification; now its results agree byte-for-byte with the actual own container invocation. No substituted inventory is used for final clearance. |
| Own/CI/author comparison | PASS | `python3 -I E/verify_local.py`; `python3 -I E/verify_ci.py` | E/verify_local.log, verify_ci.log; own/CI/author summaries identical; own/CI inventory identical; exact 36+36 and 120 test identities against candidate inventory, with 18/90/18/22/346 ECR result. |
| Integration/product suites | NOT_RUN | M2 integration/recovery/packaging acceptance, actual IPC/vault/registry/anchor/time/enforcement | Remain gated/unimplemented; models and fixture consumers are not runtime enforcement. |
| Miri/sanitizers/coverage-guided fuzzing | NOT_RUN | Pinned qualification availability probes | Reported NOT_AVAILABLE; not a safety PASS. |
| Separate standalone build wrapper | NOT_RUN separately | `bash build/scripts/build_m2_core.sh` | Fresh debug/release builds run under qualification; no additional wrapper invocation claimed. |

Reviewer setup corrections were not candidate failures: the first native oracle setup lacked cbor2/nacl, subsequently provided from exact Debian packages; an initial retirement-only reviewer prototype used nonexistent `key_class`/`epoch` attributes and raised AttributeError. Correcting that external script to the actual `authority_class`/`key_epoch` fields and rerunning both variants produced the final PASS evidence above. Neither correction edited the implementation.

## Qualification evidence

All final required subchecks PASS: source identity, Debian/amd64/non-root platform, rustc/cargo **1.85.1**, explicit package pins, full installed/deb locks, `cargo metadata --locked --offline --format-version 1`, 1454-file crate-source manifest, dependency inventory, `cargo fmt --all --check`, frozen clippy with `-D warnings`, naming checker, overflow/panic profile checks, fresh frozen debug/release builds, force-soft/fiat backends in both profiles, Valgrind probe and actual release secure-memory executable, unit suite and harness suite.

The qualification executes `env CARGO_TARGET_DIR=<Q>/build_target cargo build --workspace --frozen`, then `--release --frozen --verbose`; `valgrind --error-exitcode=99 --track-origins=yes <release probe> report`; release `cargo test --frozen -p cham3_secure_memory --no-run --message-format=json`, then the compiler-artifact-selected executable under Valgrind with `--test-threads=1`. Actual secure-memory **10 tests**, zero errors; probe zero errors. Exec'd guard/memlock child fault processes are not instrumented, unchanged limitation.

Unit: `sh tests/m2/run_m2_tests.sh unit` runs debug/release workspace tests, each **36** inventory-exact tests; provider probe/oracles and ECR **18 valid / 90 invalid / 18 admission / 22 scenarios / 346 steps**. Harness: `sh tests/m2/run_m2_tests.sh harness`, **120** inventory-exact tests. Q/U/H summaries all PASS. Exact test names were compared independently with `tests/m2/test_inventory.txt`, not inferred from totals alone.

Digest anchors:

| Artifact | SHA-256 |
|---|---|
| Own Q/summary.txt; identical CI/author summary | 5fb46465ad245f9239f35cb94954a310f48fea842d8a8e88941bdb23fcc45e4e |
| Own Q/dependency_inventory.json; identical CI inventory, 33 compiled dependencies | 7ff6c05870994425b829aed7b26c15ca70af27d35c42c98a0004c66425a17b67 |
| Own full installed package lock | 6cd45590bdebd08f53c95a56112a97030b844959983284db44232b59baa11d3c |
| Own downloaded .deb lock | 6ce06e450a8b91cd2c91c1d3aaffc360dfed7276f6622ac03dbd05fd35f0f43a |
| Cargo.lock | fc87901c5e58acdba21a33fe516871e45a81072585bfa2ae82aa8e60c40310f6 |
| Own advisory summary | 4644dcf00d1db23958577f66353c1ce82341f2439e862cc4e1cfb4d3629cf194 |
| Own debian_open.tsv | eb3b807ae8a49db107f5f039bdd88ce10c0c924d939282b7e0af36f74d7dd0e7 |
| Own I04 variant/sensitivity script | b5ffd089c054d71eb1508d9d29eef89b6d40e2b14400970104c60c79e2ea2c9e |

## Advisory judgment and every author triage group

**PASS for the current M2-001 qualification scope.** Own tracker and own inventory: **57 sources**, **108** open trixie issues (**23** not marked unimportant), **0** open rustc/rust-* issues, **0** resolved fixes newer than installed, **6** matching nonwithdrawn RustSec advisories and **0** affected versions. `debian_behind.tsv` has its header only. All open rows and six RustSec rows were inspected; an exit-0 summary by itself was not clearance. The own open TSV is byte-identical to the author's r4 open TSV despite a different retrieved tracker snapshot; all 23 author triage identifiers exactly match the current non-unimportant set.

RustSec ranges independently agree: blake2 0.10.6 ≥0.8.1; chacha20 0.9.1 ≥0.2.3; curve25519-dalek 4.1.3 ≥4.1.3; ed25519-dalek 2.1.1 ≥2; generic-array 0.14.7 falls in the ≥0.13.3 patched range; sha2 0.10.8 ≥0.9.8. Every compiled/test version was considered (33 compiled plus four declared test-only = 37). This is known-advisory clearance, not proof of dependency soundness or side-channel absence.

| Author triage group / all issue IDs | Independent judgment | Required scope limit / future action |
|---|---|---|
| cbor2 **CVE-2026-26209** (1) | **Agree for bounded repository test use.** Current tracker describes deep-nesting decoder DoS; oracle cbor2 cross-checks only already-valid, internally generated objects. Invalid inputs use the bounded cham3 decoder. | Do not carry this judgment to a decoder exposed to untrusted external input. Recheck/fix before such use. No owner risk acceptance made. |
| gcc-14 **CVE-2026-102010**, **CVE-2026-95619** (2) | **Agree for current binaries.** Tracker identifies libstdc++ heap-priority-queue erase_if and aligned C++ allocation paths. Current cham3 probes are Rust, with no C++ application layer calling these paths. | Not a blanket clearance of arbitrary C++ applications or the distribution/toolchain host. |
| glibc **CVE-2026-18374**, **19499**, **19542**, **5435**, **6238**, **6368**, **6791**, **77117**, **80489** (9; each prefix CVE-2026-) | **Agree for current M2 reachability.** Tracker identifies fopen ccs, strfmon, tdelete, resolver-printing, wordexp/tilde and specific iconv paths; current reviewed Rust/application/oracle paths do not invoke them with adversarial inputs. | Keep the judgment scoped to present probes/skeletons; reassess new locale/tree/expansion/conversion/resolver use. |
| glibc **CVE-2026-8674**, **CVE-2026-89092** (2) | **Agree today.** Resolver initialization needs name resolution; nscd issue needs that service enabled. Current M2 code performs no DNS and pinned qualification has no nscd. PrivateNetwork=yes is a future proposed unit setting, not present enforcement evidence. | Recheck if a service gains networking or deployment uses nscd; runtime isolation is not established by this review. |
| glibc **CVE-2026-97399** (1) | **Agree.** Tracker's vulnerable strncasecmp implementation is Power8-specific, outside amd64 qualification/arm64 development. | Reassess any architecture expansion. |
| glibc **CVE-2026-86805**, **CVE-2026-95818** (2) | **Agree with mandatory pre-packaging recheck.** Both are dynamic-loader issues under setuid/setgid AT_SECURE and specific $ORIGIN RPATH/RUNPATH patterns; the former additionally requires hardlink/race conditions. No current M2 setgid client is shipped. | **Before M2-007**, recheck fixes, requalify changed pins as required, inspect actual client ELF paths and relevant host protections; demonstrate that remaining triggering conditions are absent or resolved. Do not approve a future elevated client from this test-only assessment. |
| python-cryptography **CVE-2026-34073**, **CVE-2026-69248**, **CVE-2026-69249** (3) | **Agree for current oracle use.** Tracker concerns X.509 name constraints/path construction; current reviewer/provider tooling uses raw Ed25519/hashes, no certificate-chain verifier. 69248's described affected upstream range starts at 45; pin is 43, but Debian still reports the source issue open. | Neither this reasoning nor version comparison alone is a general X.509 clearance. Reassess if X.509 verification is introduced. |
| systemd **CVE-2026-15059**, **CVE-2026-16742**, **CVE-2026-40228** (3) | **Agree with deployment gate.** Tracker identifies oomd IPC path traversal, homed group escalation, and journald ForwardToWall escape handling. Current package is qualification infrastructure; M2 ships no service units, and does not use homed. | **Before M2-007**, reassess actual host units, oomd/homed deployment and journald ForwardToWall; fix/requalify or demonstrate non-applicability for the actual deployment. No claim the owner's installed system or an eventual OS image is cleared. |

The remaining 85 open rows are Debian-marked unimportant, primarily build/inspection utilities and other source-level issues. That label is not proof of impossibility: qualification uses controlled repository/build inputs, not attacker-supplied ELF archives/DWARF or arbitrary system services. No new currently reachable advisory blocker was established in the reviewed M2 scope. Future untrusted-input/deployment changes require re-review.

## I04 and earlier items

**I04 PASS.** The implementation's pending naming path is unchanged and still applies no naming effect before anchoring. The nine new observations check `named`, `min_key_epoch` and sorted retired-key IDs independently of IPC responses. They cover faulted naming, commit/status blocked by the tail, restart roll-forward, successful retry and normal first naming. The original response-preserving mutant is now rejected. The shipped new defect test requires key-state failures in both fault scenarios without response/record-count differences; removing only observations makes that test fail. Own mutations that leak **only the epoch floor** or **only pre-ceremony retirement** are detected even while `named` stays false, demonstrating that these fields add independent sensitivity. Both remain model-only checks.

**R13/R14/R15 PASS, earlier passed items intact: yes**, within the exact previously reviewed scopes. R13/R14 ceremony/role/reconciliation and R15 corrected limits are unchanged; relevant scenarios/regressions pass. Protected provider/build inputs and every ADR/spec are identical to r4. Vector bytes outside the nine observation additions are identical; signing bytes, expected results, grant standings and previous scenarios remain intact. Final qualification executes every old inventory test plus the new one.

| Earlier item | Status and retained boundary |
|---|---|
| R01-r2 | PASS: all 15 collector canaries, source identity and clean-tree qualification; unavailable/malformed hashing is still failure. |
| R02 | PASS: complete installed/deb locks reproduce exactly. |
| R03 | PASS: unchanged owned Argon2 workspace clearing/error controls and populated-library contrast in both profiles; no actual vault custody accepted. |
| R04 / R05 / R10 / R11 | PASS, intact design: denial/ledger/replay, anchored identity/freshness, global lock/order, tombstone-before-erase remain unchanged. Actual service/disk/concurrency fault tests NOT_RUN. |
| R06 / R08 | PASS, oracle/design: independent bytes and subject rejection; 61440/61441 boundary and wrappers unchanged. Actual service decoder NOT_RUN. |
| R07-r2 / R12 | PASS, model/design: network slot across branches and persistent-only ancestral restore; original sibling probe rejects and qualified controls run. Runtime network effects NOT_RUN. |
| R09-r2 / R15 | PASS, time reference model/design: maximum offset/floors and all 20 time tests; repeated-boot lag and accepted-forward-error limits honestly retained. Service clock/time-source tests NOT_RUN. |
| I01 | PASS, intact: unimplemented anchor backends and same-volume whole-restore limitation still explicit. |
| I02 | PASS: actual release secure-memory Valgrind, 10 tests/zero errors; exec'd fault children remain uninstrumented. |
| I03 | PASS, intact: same-UID, terminal/knowledge-factor and fixture limitations remain; no human-presence/isolation claim. |

## GitHub Actions comparison

**PASS, exact-target remote CI scope**, independently fetched with `gh run view <id> -R Kayyo321/cham3 --json databaseId,headSha,conclusion,status,workflowName,jobs,url` and `--log`. All seven jobs succeed at the full candidate SHA. E/ci_m2.json/log, ci_m1.json/log, ci_m0.json/log and verify_ci.log retain metadata/logs.

| Run | Jobs and independently compared evidence |
|---|---|
| m2_checks **38312981989** | qualify_amd64 **115022869104**: pins/locks/source identity, required qualification summary, 36+36 exact test identities, 18/90/18/22/346, 120 exact harness identities and both Valgrind checks agree with the final local run. CI uses --record; reviewer does not. |
| m1_checks **38312982028** | privileged_capture **115022870176**, static_and_desktop **115022870481**, package_build **115022870482**. Logs: static 82, desktop 261, staging 16, apt-pin 22, capture 77, package tests 8; successful package build. No local privileged capture/install claim. |
| m0_checks **38312982051** | static_checks **115022869153**, package_build **115022869253**, image_build **115022869285**. Static 82, packages 8, successful image build, image 17, serial login prompt, boot-result tests 18. No local ISO/VM rerun claimed. |

Artifact download/check **PASS**:

```sh
gh run download 38312981989 -R Kayyo321/cham3 -n m2_qualification_amd64 -D /home/dev/cham3-review-r5-evidence/ci_m2
gh api repos/Kayyo321/cham3/actions/runs/38312981989/artifacts
gh api repos/Kayyo321/cham3/actions/artifacts/11775706466/zip > /home/dev/cham3-review-r5-evidence/ci_m2.zip
sha256sum /home/dev/cham3-review-r5-evidence/ci_m2.zip
wc -c /home/dev/cham3-review-r5-evidence/ci_m2.zip
```

ZIP **86368 bytes**, independently computed SHA-256 **6b95c08b22272d120c2334d83ac4751cb583d8696a38c90fdf7533fd8615f345**, equals GitHub artifact metadata and author's declared digest. Each ZIP member's bytes equal the separately downloaded/extracted artifact file. Own/CI/author qualification summaries and own/CI inventories are byte-identical; summary and inventory digests are listed above. Artifact integrity verifies these bytes, not unobserved future runtime security.

## Publication and owner handoff

Only this review record is committed/pushed on `review/m2-001-002-r5-security`. Publication checks: staged whitespace, exactly one added review path against base 9cf8825, commit/push, remote review-head verification and unchanged implementation remote head **87c9cbc9bdeed40fd90b93213c22e5b854502b29**. The candidate clone remains clean/detached at 28a8595. No PR, implementation changes, merge or message to another person is part of this assignment.

Next owner action: consider the six gate recommendations and make the still-open choices in ADR 0006 §9.1 (anchor mode), §9.2 (ceremony-opening role), ADR 0007 §4 (one-shot semantics), §8 (time-bound strength/forward-error recovery), and ADR 0008 §3 (deletion granularity). None was selected. Before M2-007, perform the loader/systemd advisory and actual deployment rechecks above. Dependent tasks become READY only through their prescribed owner/dependency process. The task's later **final full-M2 review with the Codex Security plugin skill** remains required after M2-003..M2-008; this scoped qualification/design re-review does not replace it.
