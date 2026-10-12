# Independent review: M2 Rust transition

- **Verdict:** APPROVE for task/prompt/research revision and Stasha submodule removal; no actionable findings.
- **Reviewer:** Independent Codex reviewer /root/m2_packet_review, read-only. This document transcribes the returned report.
- **Environment/date:** macOS host, unprivileged Git/documentation/web inspection, 2026-10-12.
- **Branch:** codex/m2-cryptographic-contract-core.
- **Base:** c84de12d280454cba680d731f907355134a91e30.
- **Exact reviewed candidate:** a4f7b15948757906b63a90a584df6ad39bfa10a0.

The candidate follows the owner's replacement language requirement and explicit removal request. Rust is presented as the selected implementation language, without claiming the owner specifically named Rust. No core implementation exists to migrate.

The revised assignment requires substantive safe Rust domain crates with #![forbid(unsafe_code)], separately reviewed unsafe/FFI adapters, and a transitive dependency inventory. It preserves ECR architecture, immediate mandatory denial, eight work packages, fifteen acceptance criteria, G1–G6 approval gates, and separation between authorization and enforcement. It makes no new runtime or memory-safety verification claim.

Stasha's gitlink, .gitmodules registration, active local configuration, and working dependency are removed. Historical research and receipts remain unchanged apart from explicit supersession notices. Existing product authority and unrelated updates are preserved.

| Command/check | Result and evidence |
|---|---|
| git status --short; git rev-parse HEAD | PASS: clean root at exact candidate. |
| git diff --check c84de12d280454cba680d731f907355134a91e30 a4f7b15948757906b63a90a584df6ad39bfa10a0 | PASS: no formatting errors. |
| python3 /tmp/cham3_m2_rust_packet_check.py | PASS: 44 relative links; no tracked/active Stasha submodule; safe Rust assignment; eight packages/fifteen criteria; historical receipts unchanged. |
| git submodule status; git config --local --get-regexp '^submodule\.'; git ls-tree HEAD toolchains/stasha .gitmodules | PASS: no active/tracked submodule or registration. Configuration lookup returning no matches is expected. |
| git merge-base --is-ancestor c84de12d280454cba680d731f907355134a91e30 a4f7b15948757906b63a90a584df6ad39bfa10a0 | PASS: preserves baseline ancestry. |
| Exact diff inspection of PROJECT, ADRs, security invariants and testing guidance | PASS: those authority files unchanged; required architecture and gates retained in revised task. |
| Primary Rust/Cargo/Debian source inspection | PASS, research only: [Rust package](https://packages.debian.org/trixie/rustc), [Cargo package](https://packages.debian.org/trixie/cargo), [test execution](https://doc.rust-lang.org/cargo/commands/cargo-test.html), and [profile settings](https://doc.rust-lang.org/cargo/reference/profiles.html) support documented inputs and limits. |
| Rust build, Debian qualification, provider probes, product/security tests | NOT_RUN: future qualification and gated implementation. |

Future G1 evidence must exercise the actual deployed panic strategy. Cargo's ordinary test harness ignores panic=abort; unit-test success alone cannot establish abort cleanup or recovery. The task's actual-profile and process-death requirements retain this obligation. [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)

No files were edited by the reviewer. Approval covers this preparation revision only; it does not approve detailed designs, qualify a compiler/provider, or mark M2 implemented.

Coordinator disposition: the nonblocking panic-strategy note is made explicit in the task, Claude prompt and Rust research during finalization; it adds no implementation or verification claim.
