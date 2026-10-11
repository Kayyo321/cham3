# Engineering Roadmap

This is a dependency-oriented plan derived from `PROJECT.md`, not an implementation report. Repository inspection found a brief `README.md` and the root specification, but no source, build, or test implementation. Therefore all milestones below are planned, not complete.

## M0 — Repository and build baseline
- **Objective:** Establish repeatable development entry points and evidence.
- **Dependencies:** Owner confirms build host/support policy and initial repository conventions.
- **Deliverables:** Debian 13 build definition, package skeleton, CI/static checks, documented developer setup.
- **Security:** No release secrets in ordinary CI; reproducible inputs and provenance.
- **Verification/exit:** Clean checkout builds initial image/packages; record VM boot result and limitations.

## M1 — Bootable desktop distribution
- **Objective:** Deliver installable Debian 13 with KDE Plasma 6 Wayland and cham3 desktop defaults.
- **Dependencies:** M0.
- **Deliverables:** Image build, package metadata, branding, default right-side panel, install/update path.
- **Security:** Do not imply policy enforcement exists in a themed image.
- **Verification/exit:** Image build and installation/boot checks pass in a declared VM; package sources and update behavior documented.

## M2 — Contract format, key authority, and registry
- **Objective:** Verify signed, deterministic, versioned contracts and current authorization.
- **Dependencies:** Approved schema/interface decisions where PROJECT leaves detail open; M0.
- **Deliverables:** Ed25519, deterministic CBOR, COSE_Sign1 integration using maintained libraries; authority classes, registry, expiry/revocation/revision checks, vault boundary.
- **Future architecture goal (PROPOSED):** [ADR 0001 — ECR](../decisions/0001_eternal_contract_repository.md) proposes an append-only signed contract DAG, canonical audit transactions, and protected registry indexes. Resolve lifecycle/audit, schema, storage, and anti-replay decisions and obtain owner approval before dependent implementation.
- **Security:** Key custody, canonical encoding, domain separation, replay and recovery.
- **Verification/exit:** Positive and negative cryptographic/authorization tests pass; no app can request arbitrary signing.

## M3 — Isolated application launch and identity attribution
- **Objective:** Launch two same-user applications under distinct enforceable contexts.
- **Dependencies:** M1; M2 identity/contract interfaces.
- **Deliverables:** App identity registry, launcher, process lifecycle tracking, namespace/AppArmor/seccomp foundation, narrow service IPC.
- **Security:** Child/helper process inheritance and spoof-resistant attribution.
- **Verification/exit:** Cross-app files/processes/IPC and policy modification attempts are denied in documented runtime tests.

## M4 — Enforced network modes
- **Objective:** Enforce Direct, Tor, Offline per application.
- **Dependencies:** M3; approved gateway and routing interfaces.
- **Deliverables:** Network namespace manager, routing/firewall backends, Tor path and DNS handling, offline policy, transition/quarantine supervisor.
- **Security:** Tor no-fallback; DNS/IPv6/UDP, host services, reloads and partial failure.
- **Verification/exit:** Packet-observed tests demonstrate modes, gateway-failure denial, no leak during restart/transition, and positive controls.

## M5 — Trusted KDE contract workflow
- **Objective:** Inspect and authorize a running app's contract from the desktop.
- **Dependencies:** M2–M4 stable attribution and enforcement APIs.
- **Deliverables:** Super+Click path, verified window/process attribution, inspector, trusted auth UI, signed revision workflow, runtime status.
- **Future architecture goal (PROPOSED):** ADR 0001 adds trusted commit descriptions, persistent/temporary history-tree inspection, snapshot comparisons, and fresh authenticated approval, with committed state distinguished from confirmed enforcement.
- **Security:** UI is not authority; exact approved content shown; cancel and spoof tests.
- **Verification/exit:** End-to-end inspect/propose/authorize/enforce status tested without falsely displaying desired state as active.

## M6 — Live transition and revocation reliability
- **Objective:** Safely handle changed policies and active resources.
- **Dependencies:** M4–M5.
- **Deliverables:** Quarantine, socket teardown, controlled restart, revocation handling, crash recovery and status reconciliation.
- **Future architecture goal (PROPOSED):** ADR 0001 adds forward-signed historical restore under current policy, temporary-branch lifecycle handling, and transaction recovery. No stale-contract replay or delay of mandatory fail-closed denial is permitted.
- **Security:** Fail closed through all intermediate states; resource-specific limits documented.
- **Verification/exit:** Crash/restart/rollback and active access tests meet approved semantics.

## M7 — Application compatibility and multi-user hardening
- **Objective:** Support defined app classes and multi-user boundaries without weakening mandatory controls.
- **Dependencies:** M3–M6.
- **Deliverables:** Debian/Flatpak/standalone profiles, portals, filesystem/device brokers, multi-user grants, key rotation/recovery.
- **Security:** Unsupported app behavior fails visibly; cross-user isolation and session teardown.
- **Verification/exit:** Published compatibility matrix and cross-user/portal tests pass for declared support set.

## M8 — Release readiness
- **Objective:** Build, update, recover, and independently evaluate a release.
- **Dependencies:** Prior milestones and owner-approved release criteria.
- **Deliverables:** Signed APT repository, installer/recovery, release process, docs, external review preparation, known limitations.
- **Security:** Separate release keys, protected pipeline, artifact verification.
- **Verification/exit:** `docs/templates/RELEASE_GATE.md` is completed with all mandatory evidence and owner approval.

Milestone completion must be supported by repository artifacts and test evidence. Reorder only through explicit dependency review and status update.
