# Architecture Decisions

`PROJECT.md` remains the canonical product specification.

Store architecture decision records here using `../templates/ADR.md`. Mark proposals as PROPOSED until the project owner approves them. Do not use an ADR to silently replace `PROJECT.md`; record any required canonical-spec correction for owner review.

| ADR | Status | Planned scope |
|---|---|---|
| [0001 — Eternal Contract Repository and audit commit pipeline](0001_eternal_contract_repository.md) | APPROVED — scoped architecture, 2026-10-12 | Key M2 pillar: signed contract DAG, canonical transactions, protected state indexes and recovery; M5 history/approval; M6 forward restore. Lifecycle rule and canonical-spec amendments approved; detailed implementation design gates remain. |
