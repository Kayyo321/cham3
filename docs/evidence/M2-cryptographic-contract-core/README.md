# M2 evidence index

Evidence for [M2](../../tasks/M2-cryptographic-contract-core.md). Each directory names the exact commit it was produced from; files are copied verbatim from the run directories under `out/m2/` (build targets and long build logs are not committed).

| Directory | Commit | Environment | Content | Classification |
|---|---|---|---|---|
| [m2_001_arm64_f1437f0](m2_001_arm64_f1437f0/) | f1437f0d254af2968c3598bf14d1a9b2b8e791cd (clean, `dirty_tracked 0`) | Native arm64 macOS host → Docker Desktop LinuxKit 6.12.72 → pinned Debian 13.7 container (`cham3-m2-dev`), unprivileged UID 1000 | `bash build/scripts/qualify_m2_rust.sh` with `CHAM3_M2_REQUIRE_CLEAN=1`: summary, identity, toolchain, inventory, profile/naming/source-manifest logs, valgrind summary, unit (cargo test debug 32 / release 32, provider oracle, ECR vector oracle 17/80/7) and harness (57 Python tests) results | Development evidence. Not amd64 target qualification; no M2 security behavior is demonstrated because no gated domain logic exists |

amd64 target qualification: NOT_RUN at the time of writing; assigned to the independent reviewer's cloud run (see the [handoff](../../handoffs/M2-001-002-qualification-design.md)).
