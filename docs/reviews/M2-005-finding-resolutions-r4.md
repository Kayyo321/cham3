# M2-005 finding resolutions (author response, revision 4)

- **Nature of this record:** the implementer's response to the independent re-review of M2-005 revision 3. It is **not** a review and approves nothing.
- **Review answered:** `docs/reviews/M2-005-codex-security-rereview-r3.md`, commit 4865d7b7a4c885fddf71b3e0f0b14a361c3c609e on `review/m2-005-r3-security`. Its parent is 9cf882503848f8c6cc4938a588cc9fc062a9392d (`main`), it adds only that file, and its SHA-256 is `6ce7449c8784cbf67ccd5b1ee89c5da9f97c088e9001f04201cb6c4165288c50` (verified against the verdict the owner relayed). Not merged or edited.
  - It reviewed candidate be2415291bf5bcf9dad8df323bf971bd193d5120 (range 959524c..be24152, exactly the seven files) and the docs-only commit 30f77b2.
  - Verdict: **CHANGES_REQUESTED**, one new Medium finding **M2-005-R3-01**. R2-01, R2-02, R2-03 (at the binary), R01, R03 and every earlier PASS item are PASS; the author's arguments on the `<uid>` entry and on the pending reserve at start are accepted (with the cost stated as 2 + N directory syncs for N users, which this record adopts). The reviewer's qualification, protection positive path, full-filesystem run, static gates, CI comparison and the Codex Security plugin (`codex-security:security-diff-scan` 0.1.32, zero introduced findings) all ran and PASS. The M2-004 completion handoff states the M2-004 verdict and scope correctly.
- **Owner decisions:** none were needed.
- **Branch / base / candidate:** `codex/m2-cryptographic-contract-core`. Base ad18d8be68d2e5a7fd50d4577ed90edd37ce5235 (the docs-only commit recording erratum E4 and the M2-006 scope, after the revision-3 docs commit 30f77b2); revision-4 candidate **5860f276ab593da34771f704691ea35649324d6e** (one commit on the base). Commands and results: [revision-4 handoff](../handoffs/M2-005-rereview-r4.md).

## Reproduction first

The reviewer's wrapper scenario (`wrapper_run.py`: a synthetic disk directory on the overlay filesystem holding `cham3_reserve_probe_root/sentinel` and `cham3_reserve_probe_filler`; the unmodified wrapper with the candidate's probe build) was run unchanged in substance as a shell script in the pinned container, UID 1000, against a clean clone at be24152: the probe refused the filesystem (`refusing_to_fill / is overlay, not tmpfs`, exit 3), the wrapper exited 1, and **both pre-existing paths were gone** (`existing_root=deleted`, `existing_filler=deleted`), as the record reports. The same scenario at the revision-4 candidate: `refusing: … exists; this check creates and removes only its own paths`, wrapper exit 1, `existing_root=preserved`, `existing_filler=preserved`.

## Resolution matrix

| ID | Resolution | Changes | Verification |
|---|---|---|---|
| R3-01 (Medium) — the wrapper deleted pre-existing root and filler paths before the probe's filesystem refusal | The wrapper owns nothing on the disk. The unconditional `rm -rf` before the probe is gone. Before anything is built or run, an existing root or filler path — a file, a directory or a symbolic link, dangling or not (`-e` or `-L`) — refuses the whole check with exit 1 and a message naming the path; nothing is touched. The probe itself already refused such paths (R2-03); the wrapper now refuses them first. The leftover checks after each profile also cover symbolic links. For testability the repository root and the cargo target directory may be given in the environment (`CHAM3_REPO_ROOT`, `CHAM3_M2_RESERVE_TARGET_DIR`), so the harness runs the real script against the dispatcher's probe build. The alternative the review offered, a private per-invocation namespace, does not cover the filler, whose path at the mount root is the probe's; refusal does. | `tests/m2/storage/check_reserve_full_disk.sh` | **Harness tests** (`test_m2_reserve_probe.py`, `full_disk_wrapper_t`): an existing root with a sentinel and an existing filler are refused and preserved byte for byte; symbolic links for both paths, with existing and with missing targets, are refused with the links and their targets preserved; a disk without the paths is left as found (an ordinary filesystem is refused by the probe; on the CI tmpfs the positive path runs). **Seeded wrapper defect** (`seeded_wrapper_defects_t`): the baseline pre-deletion restored in a copy of the script is caught by the preservation case (on an ordinary filesystem the probe then refuses, on a tmpfs it runs; the lost data is the detection either way); the unmodified copy preserves. **On the tmpfs:** the CI full-filesystem job and the author's VM run the five wrapper cases against the 2 MiB tmpfs after the positive path, with `CHAM3_M2_WRAPPER_DISK=/full-disk` (each case uses a fresh subdirectory it creates and removes); no leftovers. **Reviewer's scenario** at the candidate: both paths preserved. |

## Scope of the change

- `tests/m2/storage/check_reserve_full_disk.sh` (the fix); `tests/m2/harness/test_m2_reserve_probe.py` (+5 tests: 3 wrapper cases, the seeded wrapper defect and its control); `tests/m2/test_inventory.txt` (+5); `.github/workflows/m2_checks.yml` (the wrapper cases on the tmpfs after the full-filesystem step).
- **Not changed:** any Rust source, dependency, `Cargo.lock`, manifest, specification, ADR, oracle or vector.

## Not changed, and carried forward

- Everything the revision-3 record found intact stays as it was; the stage A work of M2-006 is not in this candidate (it continues on the branch after this re-review).
- The reviewer's qualification needed a container-only `--ulimit nofile=1024:1024` for Valgrind on the home computer; environment setting, not a code change.
- Nothing is merged; the review record is not edited.
