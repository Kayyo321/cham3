# M2-001 / M2-002 revision 4.1 — native amd64 evidence at 28a8595 (author VM)

- **Candidate:** 28a8595e627bcd092acaff4b8161522cf5d38f7f, tree ad08a16302ea51d8b9f9322ca6599b50c987f2ea (parent 70c431c13a1c76fd2948e6c8f0ae33b720618043). This is a test- and oracle-only change answering I04.
- **Environment:** the same as [revision 4](../m2_001_amd64_vm_9b1aa2b/):
  - Anthropic cloud VM, native x86_64, kernel 6.18.44.
  - `cham3-m2-dev` built from the unchanged Dockerfile, every step CACHED (Debian 13.7 amd64).
  - UID/GID 1000, clean clone owned by UID 1000.
  - No `--privileged`, no added capabilities, no `--record`.
- **Classification:** author development evidence. The second run is GitHub Actions ([CI evidence](../m2_001_amd64_ci_28a8595/)). Neither is an independent review.

The commands are those of revision 4, with the clone at `/tmp/cham3_r41_run` checked out at 28a8595.

| File | Content |
|---|---|
| `summary.txt`, `qualification_console.txt` | Every required check PASS, exit 0. Miri, sanitizers and fuzzing are NOT_AVAILABLE (NOT_RUN). The summary differs from revision 4 only in its commit line and is identical to the CI summary. |
| `identity.txt` | `dirty_tracked 0`, Cargo.lock `fc87901c…10f6`, EUID 1000 |
| `installed_packages_diff.log`, `downloaded_debs_diff.log` | Empty diffs against the committed amd64 locks |
| `unit/` | 36 + 36 cargo tests; provider oracle 7/7; ECR oracle 18/90/18 with 22 journal scenarios (346 steps), including the new key-state observations |
| `harness/` | **120 tests** (119 carried + 1 new I04 defect test) |
| `valgrind_*_summary.txt`, `static_checks.log`, `static_tests.log` | memcheck: 0 errors, 10 secure-memory tests. Static checks PASS; 82 static tests OK; the clone was clean afterwards. |
| `unchanged_inputs.txt` | `git diff --exit-code 9b1aa2b 28a8595 -- core Cargo.toml Cargo.lock build .github PROJECT.md docs/decisions docs/specs tests/m2/run_m2_tests.sh tests/m2/lib` exits 0: no ADR, spec, script or Rust change. In `vectors.json`, only `keys` observations were added to 9 steps of the 3 `recovery_naming_*` scenarios; 19 of 22 scenarios and every other section are identical. |
| `reviewer_r4_reproductions/` | The re-review 3a3b5d1's `early_effect_probe.py`, extracted unchanged from the review record (SHA-256 `2cc05d5a…8358`):<br>• at 9b1aa2b it reports zero scenario failures (exit 0), which reproduces I04;<br>• at 28a8595 the scenarios report `key state` failures and the probe's `assert not failures` fails (exit 1), so the early effect is now detected. |

Limits: these are model and test evidence only. Real IPC, vault, registry and anchor durability are NOT_RUN.
