# Dependency advisory review, revision 3 (author evidence)

Repeats the [revision-2 author review](../advisory_review_20261012/advisory_readme.md) for ADR 0003 §11, as requested for the revision-3 candidate. The independent re-review (336f13b) could not retrieve the Debian tracker (BLOCKED, HTTP 403) and independently confirmed only the RustSec part. **This is implementer evidence, not an independent clearance**; the reviewer's own Debian check remains required before G1 approval.

| Input | Identity |
|---|---|
| Debian security tracker | `https://security-tracker.debian.org/tracker/data/json`, fetched 2026-10-12 18:29 UTC from the author VM (`curl -sS --fail`), 82 447 256 bytes, SHA-256 `52d2faead64ec0a05432d60ffdda51d74c298dabb633ac8146e0356d7be65dc6` — byte-identical to the revision-2 snapshot |
| RustSec advisory-db | `git clone https://github.com/RustSec/advisory-db.git`, commit `57dcc0f2192e025138907a495c1ac9a7e1e3e123` (2026-10-12T20:06:48+02:00) |
| Installed packages | pinned `cham3-m2-dev` image built natively on amd64 from the unchanged Dockerfile (all build layers identical to the baseline build) |
| Crate set | `dependency_inventory_amd64.json`: `cargo metadata --locked --offline` + `m2_dependency_inventory.py inventory` in that image on a clean clone (33 compiled crates; dependency inputs are unchanged by revision 3), plus the 4 declared test-only crates |

Command (container, UID 1000, inputs mounted outside the repository):

~~~sh
python3 -I build/scripts/m2_advisory_review.py --tracker /adv/tracker/debian_tracker.json \
  --advisory-db /adv/advisory-db --inventory /adv/inv/dependency_inventory.json --out-dir /adv/out
~~~

Result: exit 0 (`summary.txt`). 57 source packages; **0** resolved fixes newer than installed (`debian_behind.tsv` header only); **0** open issues in `rustc`/`rust-*` sources; 108 open trixie issues, 23 not marked unimportant (`debian_open.tsv`, identical to revision 2); RustSec: 6 advisories, all patched in the pinned versions, **0 affected** (`rustsec.tsv`, identical to revision 2).

Triage of the 23 open items (`triage.tsv`): 17 not applicable to the current M2 binaries (C++ library, glibc functions the Rust code never calls, nscd, a ppc64le-only variant, X.509 in a test-only library); cbor2 is test-oracle-only on repository-generated input; the two glibc `ld.so` items and the three systemd items are **preconditions to re-check before M2-007 packaging** (setgid client, service units). No item blocks M2-001/M2-002; none is cleared for packaging.

Limits: advisory databases are snapshots; absence of an advisory proves neither soundness nor security; applicability was judged by the author and needs independent confirmation.
