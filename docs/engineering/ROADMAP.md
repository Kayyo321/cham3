# Engineering Roadmap

This is a dependency-oriented plan derived from `PROJECT.md` and scoped owner-approved ADRs, not an implementation report. See `../STATUS.md` for exact implementation and verification evidence. ADR 0001 received scoped owner approval on 2026-10-12; M2 and later ECR work remain unimplemented.

## M0 — Repository and build baseline
- **Objective:** Establish repeatable development entry points and evidence.
- **Dependencies:** Owner confirms build host/support policy and initial repository conventions.
- **Deliverables:** Debian 13 build definition, package skeleton, CI/static checks, documented developer setup.
- **Security:** No release secrets in ordinary CI; reproducible inputs and provenance.
- **Verification/exit:** Clean checkout builds initial image/packages; record VM boot result and limitations.

## M1 — Bootable desktop distribution
- **Objective:** Deliver an installable Debian 13 (Trixie) x86-64 image with KDE Plasma 6 on Wayland, cham3 branding, the default right-side panel, and a curated desktop/security application suite.
- **Dependencies:** M0 build baseline; approved panel composition in [ADR 0002](../decisions/0002_default_right_taskbar_layout.md); package-by-package source and dependency review under the owner-approved policy recorded in the [M1 task specification](../tasks/M1-bootable-desktop-distribution.md).
- **Deliverables:** Debian live-build image and installer path; `cham3-desktop` and `cham3-full` package metadata with the curated security suite in the default `cham3-full` set; cham3 styling; the visual/package foundation for the nine-component 56–64 logical-pixel panel defined in [ADR 0002](../decisions/0002_default_right_taskbar_layout.md) and scoped by the [M1 task specification](../tasks/M1-bootable-desktop-distribution.md); LibreWolf and Tor Browser; Wireshark/tshark, proxy and web auditing tools, CLI traffic monitors, and a curated security-tool suite; categorized launcher entries; documented package sources and update behavior. Prefer Debian packages, use Debian-native equivalents for unavailable/incompatible tools, and allow selected Kali-origin packages only through narrowly pinned, separately reviewed sources where no suitable native option exists. Preserve Debian KDE utilities and prune duplicate file managers, terminals, editors, archive managers, viewers, and screenshot/resource utilities. Full trusted panel interactions are M5 scope under M5-001.
- **Security:** M1 provides desktop layout, UI scaffolding, and package configuration only for M2–M4 backend-dependent features. Missing backends must show unavailable/disconnected status; no visual control may imply active cryptographic authorization, sandboxing, network-mode enforcement, or kill-switch isolation.
- **Verification/exit:** Build an installable ISO from declared Debian 13 x86-64 inputs; boot and install it in a declared disposable VM; verify Plasma Wayland, branding, panel layout, categorized launchers, package sourcing, and duplicate-utility absence. Record exact evidence and limitations. This exit does not establish M2–M4 enforcement.

## M2 — Cryptographic contract core and Eternal Contract Repository
- **Objective:** Verify signed, deterministic, versioned contracts and current authorization, with ECR as the durable contract and change-history foundation.
- **Key pillar — ECR (APPROVED architecture):** [ADR 0001](../decisions/0001_eternal_contract_repository.md) makes an append-only signed contract DAG, canonical authorization transactions, protected current-state indexes, and rollback-resistant recovery required M2 deliverables. ECR is part of the milestone's core acceptance, not an optional later history feature.
- **Dependencies / design gates:** M0; approved ADR 0001 and `PROJECT.md` §5.7 / Phase 2; independently reviewed, owner-approved wire/schema/hash profile, registry/vault and lifecycle-evidence interfaces, storage/atomicity/concurrency, rollback-resistant anchors and backup/recovery, retention/privacy, and authentication/identity-transition decisions. Gate affected implementation tasks until these are settled.
- **Deliverables:** Per-user identities and persistent/temporary key authorities; encrypted vault boundary; maintained-library Ed25519, deterministic CBOR and COSE_Sign1; immutable persistent/temporary DAG objects; validated genesis/parent/merge/state/delta admission; atomic objects and pending/current-state indexes; expiry/revocation/one-shot/revision evaluation; lifecycle/audit-gap reporting and replay-resistant crash recovery. M5 supplies the trusted history UI; M6 supplies runtime forward-restore and transition integration.
- **Security:** A durable signed commit precedes new or expanded authority; signature, current authorization, admission, and confirmed enforcement stay distinct. Mandatory denial cannot wait for signing, authentication, or history writes. Preserve app isolation, key custody, domain separation, current-policy precedence, and no stale-state reactivation.
- **Verification/exit:** Positive/negative cryptographic and authorization vectors; valid/invalid DAG admission; stale-head and duplicate/consumed-grant rejection; concurrent and interrupted-write recovery; stale-index/tail/anchor rollback cases; locked-vault, missing-user, and unavailable-storage denial cases. Require exact commands/artifacts and independent security review under [TESTING.md](TESTING.md). No arbitrary app signing, no incomplete transaction grants, and no mandatory-denial delay. Unit/component results do not establish later KDE, sandbox, or networking enforcement; M2 cannot exit with only signature verification or a mutable basic registry.

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
- **ECR integration (approved ADR 0001):** Trusted commit descriptions, persistent/temporary history-tree inspection, snapshot comparisons, and fresh authenticated approval, with committed state distinguished from confirmed enforcement. Depends on M2 ECR and approved interfaces.
- **Security:** UI is not authority; exact approved content shown; cancel and spoof tests.
- **Verification/exit:** End-to-end inspect/propose/authorize/enforce status tested without falsely displaying desired state as active.

## M6 — Live transition and revocation reliability
- **Objective:** Safely handle changed policies and active resources.
- **Dependencies:** M4–M5.
- **Deliverables:** Quarantine, socket teardown, controlled restart, revocation handling, crash recovery and status reconciliation.
- **ECR integration (approved ADR 0001):** Forward-signed historical restore under current policy, temporary-branch lifecycle handling, and transaction recovery. Depends on M2 admission/recovery and M5 trusted approval; no stale-contract replay or delay of mandatory fail-closed denial is permitted.
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
