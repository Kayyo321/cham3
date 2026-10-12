# Architecture Decisions

`PROJECT.md` remains the canonical product specification.

Store architecture decision records here using `../templates/ADR.md`. Mark proposals as PROPOSED until the project owner approves them. Do not use an ADR to silently replace `PROJECT.md`; record any required canonical-spec correction for owner review.

| ADR | Status | Planned scope |
|---|---|---|
| [0001 — Eternal Contract Repository and audit commit pipeline](0001_eternal_contract_repository.md) | APPROVED — scoped architecture, 2026-10-12 | Key M2 pillar: signed contract DAG, canonical transactions, protected state indexes and recovery; M5 history/approval; M6 forward restore. Lifecycle rule and canonical-spec amendments approved; detailed implementation design gates remain. |
| [0003 — M2 Rust toolchain, dependencies and unsafe boundary](0003_m2_rust_toolchain_dependencies.md) | PROPOSED (G1, revision 3), 2026-10-12 | Debian 13 Rust 1.85.1 pin, Debian-packaged offline crates and source manifest, portable crypto backends, single reviewed unsafe adapter, profiles/panic policy, harness tooling. |
| [0004 — M2 wire schema, deterministic encoding and hashes](0004_m2_wire_schema_hash.md) | PROPOSED (G2, revision 3), 2026-10-12 | `cham3_dcbor_v1`, COSE_Sign1 profile (Ed25519 −19, `typ`, external_aad), hashes, commit payload/types, evolution; spec, CDDL and independent-oracle vectors. |
| [0005 — M2 identity, authentication, vault and service IPC](0005_m2_identity_vault_ipc.md) | PROPOSED (G3, revision 3), 2026-10-12 | Identity sources, key hierarchy, passphrase approval bound to exact bytes, encrypted key objects and KDF bounds, vault hardening, five-process SEQPACKET IPC with peer-credential roles. |
| [0006 — M2 ECR storage, anchors and recovery](0006_m2_storage_anchor_recovery.md) | PROPOSED (G4, revision 3), 2026-10-12 | Per-user chained append-only journal, derived indexes, durability order, anchor backends and assumptions, recovery/quarantine table, staged recovery ceremony; owner choices of default anchor mode and of who opens the ceremony. |
| [0007 — M2 lifecycle evidence, time and consumption](0007_m2_lifecycle_time_consumption.md) | PROPOSED (G5, revision 3), 2026-10-12 | Current-authorization rules, restriction-first revocation, per-lineage network slot, unsigned lifecycle provenance and audit gaps, BOOTTIME maximum-offset trusted time, sessions; owner choice of one-shot semantics. |
| [0008 — M2 retention, privacy and update identity](0008_m2_retention_privacy_update_identity.md) | PROPOSED (G6, revision 3), 2026-10-12 | Quotas/full disk, history access, deletion and tombstones, export/backup, migrations, identity transitions after updates; owner choice of deletion granularity. |
