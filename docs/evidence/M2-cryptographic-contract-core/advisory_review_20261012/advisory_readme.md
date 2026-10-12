# Dependency advisory review, 2026-10-12 (author-run)

Requested by the independent review (9cf8825), whose own retrieval was BLOCKED (HTTP 403). This is implementer evidence for ADR 0003 §11 and must be repeated by the reviewer before G1 approval.

| Input | Identity |
|---|---|
| Debian security tracker | `https://security-tracker.debian.org/tracker/data/json`, fetched 2026-10-12 17:07 UTC, 82 447 256 bytes, SHA-256 `52d2faead64ec0a05432d60ffdda51d74c298dabb633ac8146e0356d7be65dc6` |
| RustSec advisory-db | `https://github.com/rustsec/advisory-db` commit `25a288f68f2d5ebe658b243a334110c7217a2569` (2026-10-12T13:40:46+02:00) |
| Installed packages | pinned `cham3-m2-dev` container (arm64 run). The 66 relevant packages (compiled-crate sources, explicit pins, toolchain/oracle runtime libraries) have identical versions in `build/m2/debian_lock_amd64.txt` and `debian_lock_arm64.txt` |
| Crate set | `dependency_inventory.json` of the f1437f0 qualification (33 compiled crates) plus 4 declared test-only crates |

Command (inside the container, inputs mounted read-only):

~~~sh
python3 -I build/scripts/m2_advisory_review.py --tracker tracker.json --advisory-db advisory-db \
  --inventory docs/evidence/M2-cryptographic-contract-core/m2_001_arm64_f1437f0/dependency_inventory.json \
  --out-dir docs/evidence/M2-cryptographic-contract-core/advisory_review_20261012
~~~

Result (exit 0; `summary.txt`):

- 57 source packages checked. **0** resolved trixie fixes newer than the installed versions (`debian_behind.tsv` is header-only).
- **0** open trixie issues in `rustc` or any `rust-*` source of the compiled crates.
- 108 open trixie issues in the checked sources, 23 not marked "unimportant", all with urgency "not yet assigned": glibc (13, including two `ld.so` items), gcc-14 libstdc++ (2), systemd (3) and the test-only oracles cbor2 (1) and python-cryptography (3) (`debian_open.tsv`). None is in code the current M2 Rust binaries call; the `ld.so` items are a precondition to re-check before the setgid client is packaged (M2-007); systemd items are re-checked with service packaging.
- RustSec: 6 advisories exist for the checked crates (blake2 RUSTSEC-2019-0019, chacha20 RUSTSEC-2019-0029, curve25519-dalek RUSTSEC-2024-0344, ed25519-dalek RUSTSEC-2022-0093, generic-array RUSTSEC-2020-0146, sha2 RUSTSEC-2021-0100); the pinned versions satisfy every patched range; **0 affected** (`rustsec.tsv`).

Limits: advisory databases are snapshots; absence of an advisory is not a soundness or security proof; applicability of open distribution issues was judged by the author and needs independent confirmation.
