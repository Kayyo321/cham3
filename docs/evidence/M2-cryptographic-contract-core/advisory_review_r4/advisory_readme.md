# Dependency advisory review, revision 4 (author evidence)

This repeats the [revision-3 author review](../advisory_review_r3/advisory_readme.md) for ADR 0003 §11, at the revision-4 candidate 9b1aa2b9d1435fb33c303ca7fe01ce7afcb2964a. The independent re-review f451f2e could again not retrieve the Debian tracker (BLOCKED, HTTP 403 in the reviewer's environment). It confirmed only the RustSec part. **This is implementer evidence, not an independent clearance.** The reviewer's own Debian check, or an owner decision on how G1 is cleared, is still required.

| Input | Identity |
|---|---|
| Debian security tracker | `https://security-tracker.debian.org/tracker/data/json`, fetched 2026-10-12 19:40 UTC from the author VM (`curl -sS --fail`). 82 482 238 bytes, SHA-256 `005cdba6d320b0410025ec9dbd0bea12a56ba4b01b416295394bfd291fa3b64b` (a newer snapshot than revision 3's `52d2faea…5dc6`) |
| RustSec advisory-db | `git clone https://github.com/RustSec/advisory-db.git`, commit `57dcc0f2192e025138907a495c1ac9a7e1e3e123` (2026-10-12T20:06:48+02:00; unchanged) |
| Installed packages | The pinned `cham3-m2-dev` image, built natively on amd64 from the unchanged Dockerfile |
| Crate set | `dependency_inventory_amd64.json` from the revision-4 qualification (33 compiled crates), byte-identical to revision 3's inventory, plus the 4 declared test-only crates |

Command (container, UID 1000, candidate tree read-only, inputs mounted outside the repository):

~~~sh
python3 -I build/scripts/m2_advisory_review.py --tracker /adv/tracker/debian_tracker.json \
  --advisory-db /adv/advisory-db --inventory /adv/inv/dependency_inventory.json --out-dir /adv/out
~~~

Result: exit 0 (`summary.txt`):
- 57 source packages;
- **0** resolved fixes newer than installed;
- **0** open issues in `rustc`/`rust-*` sources;
- 108 open trixie issues, of which 23 are not marked unimportant;
- RustSec: 6 advisories, **0 affected**.

`debian_behind.tsv`, `debian_open.tsv` and `rustsec.tsv` are **byte-identical** to revision 3 despite the newer tracker snapshot. The revision-3 item-by-item triage therefore applies unchanged (`triage.tsv`, copied):
- 17 items are not applicable to the current M2 binaries;
- cbor2 is used by the test oracle only;
- the glibc `ld.so` and systemd items are preconditions to re-check before M2-007 packaging.

No item blocks M2-001/M2-002, and none is cleared for packaging.

Limits:
- Advisory databases are snapshots.
- Absence of an advisory proves neither soundness nor security.
- Applicability was judged by the author and needs independent confirmation.
