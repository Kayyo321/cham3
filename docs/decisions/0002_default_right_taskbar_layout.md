# ADR 0002: Default right-hand taskbar layout and trusted desktop boundaries

- **Status:** APPROVED
- **Date / decision owner:** 2026-10-11 / cham3 project owner (requesting user).
- **Related task/spec sections:** [M5-001](../tasks/M5-default-taskbar-layout.md); `PROJECT.md` §§2.5, 3, 4.4–4.6, 6–9, 11, 12.2, 14–18; roadmap M1, M4, M5, M6; SEC-01–16, SEC-18.

`PROJECT.md` is the canonical product specification. This ADR records a scoped decision or proposal and does not silently replace that document.

## Problem and context

`PROJECT.md` §8 requires a default right-side Plasma panel, per-application network indicators, trusted contract inspection, and standard desktop functions. The owner has finalized the panel composition and interactions and requested an approved ADR and a KDE milestone task. This record formalizes that decision; it does not assert that the panel, services, isolation, or enforcement are implemented.

The engineering roadmap assigns the bootable desktop to M1, network enforcement to M4, and trusted KDE integration to M5. The full panel belongs to M5, with M1 supplying its visual/package foundation and M4/M6 supplying enforcement dependencies. The suggested M4 task filename in the request is illustrative, not a milestone renumbering.

[ADR 0001](0001_eternal_contract_repository.md) received scoped architecture approval on 2026-10-12. Its detailed schema, lifecycle evidence, storage, and recovery designs remain implementation gates; this ADR approves no such details. This dependency-status correction changes none of the panel or emergency-denial boundaries below.

## Decision

### Geometry and implementation

Ship a vertical dock anchored to the right-hand screen edge, 56–64 px wide. Interpret panel geometry in Plasma logical pixels and verify physical scaling separately. Use dark surfaces, geometric controls, restrained effects, and Rose / Purple / White indicators for Direct / Tor / Offline. Labels and accessible descriptions must convey status without relying only on color.

Compose the panel from modular Plasma 6 QML plasmoids under KWin Wayland, packaged through `cham3-desktop`. Use separate utility windows and System Settings integrations where specified below. Plasmoids render state and submit bounded requests; they do not hold private keys, evaluate final authorization, or execute privileged firewall commands. Extend upstream KDE through maintained extension points and narrowly scoped KWin integration; a complete KDE fork is outside this decision.

Upstream documentation describes [Plasma widget packaging](https://develop.kde.org/docs/plasma/widget/setup/), [KWin scripts](https://develop.kde.org/docs/plasma/kwin/), and [QML effects with window thumbnails](https://develop.kde.org/docs/plasma/kwineffect/). These support the extension approach; they do not establish availability of every needed API in the selected Debian/Plasma build. Pin versions and verify input interception, modifier-release handling, window moves, and compositor previews before implementation readiness. A narrow trusted native integration may be necessary; its exact API and privileges require review.

New internal identifiers use `snake_case`, constants/macros use `PascalCase`, and types use `snake_case_t`; no internal `camelCase`. Required upstream QML/Qt/KDE names and canonical hyphenated service names remain at documented external boundaries. No upstream public interface is renamed by this ADR.

### Top-to-bottom component order

| Order | Component | Required presentation and behavior |
|---|---|---|
| 1 | `cham3_user_launcher` | Circular cropped avatar of the authenticated human user at the top. Left-click opens a Windows-style application/session flyout: search, categorized applications with contract badges, public user `key_id`, and Lock, Log Out, Reboot, Power Off controls. |
| 2 | `cham3_alias_wheel` | Trifold donut dial: Rose Direct/Clearnet, Purple Tor, White Offline. Clicking a segment immediately selects that execution alias. `Super + Tab` opens a central HUD with live miniature desktop previews of all three aliases; repeated `Tab` cycles, releasing `Super` commits selection. |
| 3 | `cham3_tag_pager` | Five dwm-style virtual monitor tags, `1`–`5`, per alias: 15 contexts total. Outline = empty; outline plus dot = occupied; solid fill = active; pulsing border = urgent. Left-click changes the viewed tag; `Super + Click` moves the active window to that tag. |
| 4 | `cham3_contract_dock` | A 2×2 grid: top-left Contract Handler (`cham3_contract_manager` standalone utility); top-right ECR Explorer (`cham3_ecr_explorer`); bottom-left Active Contracts Monitor (`cham3_active_monitor`, live process/sandbox state and revocation); bottom-right Hot Contract View toggle. |
| 5 | `cham3_netproto_launcher` | Full-width horizontal routing-glyph button directly below the grid; opens the Unified 3-Tab Network Protocol Editor described below. |
| 6 | `cham3_killswitch` | Physical-style rocker. Off requests Full Isolation Kill; display confirmed enforcement separately from the requested position and from pending/failure state. |
| 7 | `cham3_settings_launcher` | Gear icon opens the cham3 Unified Control Center: KDE System Settings with dedicated cham3 security sections prominently presented. |
| 8 | `cham3_task_manager` | Two scoped icons-only sections with a subtle divider: active-tag applications, followed by dimmed applications on other tags within the current alias. Detailed behavior below. |
| 9 | Upstream Plasma tray and clock | Standard notifications, volume, battery, and trusted Tor circuit status; vertically formatted digital clock/calendar at the bottom edge. |

### Aliases, tags, and launch authority

An alias is a user-scoped execution context with independent browser/application data directories, an intended default contract tier, and five tag contexts. These are three aliases within one human user's desktop; they do not create three signing identities, virtual machines, or separate mandatory desktop environments. This refines §8.2 without reviving the discarded multi-VM design.

Alias selection is immediate desktop navigation and chooses the context for subsequent launches. It never retargets existing processes, copies profiles across aliases, changes an active contract, or changes an application's enforced network mode. The launch workflow verifies the application, selected alias/profile, effective contract, and mandatory policy before execution. Missing or invalid network authorization still starts Offline (SEC-07); selecting the Rose segment cannot grant Direct access. Any actual mode change follows the authenticated contract workflow and initial controlled restart in `PROJECT.md` §7.7–7.8, including the Tor-to-Direct warning and separate data profile where needed.

Alias-private storage must be enforced by launcher/filesystem/broker boundaries, including helper processes. Separate directory names, Plasma Activities, virtual desktops, tag filters, and QML visibility alone provide no security isolation. The backend mapping from alias/profile to verified sandbox identity remains to be specified before implementation. An alias's color denotes its default tier; each application pip denotes its individually confirmed enforced mode, which can differ. Explain that difference in the UI.

Tags are view/focus metadata scoped to the current alias. Moving a window between tags preserves its verified sandbox, data profile, effective contract, and network mode. The pager move and background-task pull operate within the current alias only. Tag operations cannot move a running process into another alias or confer authority. Urgency is an untrusted attention hint, never an authorization signal.

The alias HUD uses trusted compositor previews scoped to the authenticated user's current session. It does not grant applications screen capture, export preview buffers to clients, or expose other users/locked sessions. Clear previews on lock/session change and do not persist them. The exact compositor capability boundary and handling of protected/non-previewable surfaces require review before implementation. `Escape` cancels HUD selection without changing the committed alias.

### Contract tools and Hot Contract View

The Contract Handler manages proposals and verified contracts through contract/policy services. The ECR Explorer is a read-only DAG history viewer dependent on implementation of approved ADR 0001 and its detailed design gates. Until available, its reserved launcher visibly reports the dependency and cannot imply valid history or provide a fabricated DAG. The Active Contracts Monitor binds processes/sandboxes to current authorization and confirmed resource enforcement; revocation uses trusted registry/broker/runtime operations and discloses limits for already copied data and non-revocable resources.

Hot Contract View arms a latched button and inspection-reticle cursor. Trusted KWin hover highlights the managed window. Left-click consumes the selection, resolves verified window → application → runtime sandbox through `cham3-sessiond` and launcher attribution, opens `cham3_inspector`, and auto-disarms. `Escape` cancels; lock, session loss, target destruction, or attribution failure clears the inspection state. Unresolved attribution displays an explicit unknown/error state and cannot open another application's authority. Inspection requires no vault unlock/signing.

Preserve the foundational `Super + left-click` on application windows from `PROJECT.md` §8.3 and provide keyboard-only and taskbar/context-menu equivalents. Dispatch modifier clicks by the actual trusted target: a pager tag moves the active window, a background task icon pulls its represented window, and an application surface opens its inspector. Consume handled events to prevent double action or delivery as application approval input. Desktop shortcuts must resolve upstream conflicts explicitly.

### Unified 3-Tab Network Protocol Editor

| Tab | Exposed configuration scope |
|---|---|
| Clearnet | DNS resolvers, upstream gateways, interface bindings, IPv6/IPv4 fallback subject to Direct policy. |
| Tor | Managed daemon configuration, bridges/pluggable transports, entry guard and exit-selection policy, circuit refresh, and SOCKS ports. Unsupported or unsafe options are rejected. No automatic Direct fallback is permitted. |
| Offline | Strict loopback rules, peripheral network isolation, and domain socket access policies, subordinate to mandatory isolation and essential authorized IPC. |

The editor shows proposed settings, effective configuration, pending application/restart requirements, and confirmed enforcement/failure independently. It submits typed, bounded proposals to trusted policy/contract services. They validate ownership, mandatory restrictions, configuration semantics, listeners/routes, and affected sandboxes, and obtain explicit approval through `cham3-authui`. Only that validated authenticated workflow may request cryptographic authorization from `cham3-vaultd`; the plasmoid/editor has no arbitrary signing API.

`cham3-netd` owns the enforcement transaction and nftables/namespace updates. The vault signs approved content; it does not administer networking. Apply changes with quarantine, existing connection teardown, and controlled restart where required; report active settings only after affected enforcement domains confirm them. Failure leaves affected networking denied. Raw configuration injection, arbitrary privileged commands, broadly exposed SOCKS listeners, IPC proxy bypasses, and edits that weaken Offline/Tor restrictions are prohibited. IPv4 fallback is confined to an authorized routing tier and cannot turn Tor failure into clearnet traffic.

The signed configuration schema, policy scope, service operations, Tor option allowlist, device-control ownership, durable transaction/recovery details, and treatment of settings outside the existing contract schema are unresolved dependencies. This ADR approves the editor boundary and workflow, not an invented wire format or new signing primitive. Define them in reviewed, owner-approved interface decisions before implementing backend writes.

### Global Internet Kill Switch

Off means **Full Isolation Kill**, a mandatory global deny overlay: deny all WAN and LAN traffic and all IP loopback TCP/UDP, including `127.0.0.1` and `::1`. Cover host and application namespaces, IPv4/IPv6, ingress/egress/forwarding, existing connections, wired/wireless/peripheral paths, and IP protocols beyond TCP/UDP so alternative traffic cannot bypass the kill. Do not let established-connection rules override the deny. Apply host-controlled nftables/namespace enforcement and rfkill for supported radios; rfkill alone cannot isolate wired paths or loopback.

Preserve `AF_UNIX` local IPC for authorized Wayland/KWin/D-Bus and recovery/control operations. This preserves the socket family, not unrestricted access to every Unix socket. Existing per-application IPC controls remain active, and a host service reached by Unix socket cannot proxy network traffic around the global deny. Tor gateway IP traffic is denied too; the tray must distinguish configured Tor state from usable, confirmed routing.

An authenticated human can request emergency denial without unlocking the vault, obtaining a signature, or waiting for ECR availability. Denial must not wait for history writes or an authorization step that grants no authority. Only trusted enforcement services may apply the overlay. An application cannot flip the switch or synthesize approval. Re-enabling connectivity is a separate validated, appropriately authenticated workflow; it removes only the global deny overlay and restores no expired, revoked, unsigned, or otherwise unauthorized grant. It never converts Tor to Direct or automatically expands contract authority.

While enforcement is pending or cannot be confirmed, report pending/unknown/failure and keep the deny or quarantine boundary. Boot/session startup, firewall reload, service crashes, suspend/resume, and interface/namespace creation must not create an unguarded path. Exact persistent kill-state storage, boot ordering, radio reconciliation, failure containment, and restore-authorization binding must be decided and independently tested before implementation readiness; no full-system isolation claim follows from rocker position or a ruleset alone.

### Dual-section task manager

- **Section A — Active Tag Apps:** icons-only tasks strictly filtered by verified current alias and current tag. Network-tier corner pips show confirmed per-application Rose/Purple/White modes, with distinct pending, unknown, and failure states.
- **Section B — Background Tag Apps:** windows on other tags `1`–`5` in the current alias, separated by a subtle divider, at 70% opacity with a solid circular corner badge showing the tag number. Left-click jumps to that tag and focuses the window. `Super + Click` pulls it to the current tag without changing the viewed context or sandbox authority.

Exclude other aliases/users from both task sections. Resolve every action against a current verified runtime/window binding; stale or ambiguous mappings cannot select a different sandbox. Show unclassified windows through a clearly marked diagnostic/unknown path rather than assigning them a compliant color or hiding an enforcement failure. Placement/filtering of trusted system utility windows must be settled in the interface specification. Controls remain keyboard accessible and urgency has a non-pulsing equivalent when reduced motion is enabled.

### Logical interfaces and component boundaries

These are required responsibilities and validation rules, **not frozen IPC method names or a claim of existing service APIs**.

| Consumer / operation | Trusted boundary | Required input/output and denial behavior |
|---|---|---|
| User launcher / session controls | `cham3-identityd`, authenticated session control, `cham3-launcherd` | Resolve the logged-in human and public `key_id`; launch only the verified application/profile under effective policy. Session actions use existing authenticated system mechanisms. Never expose secrets. |
| Alias wheel / pager / task manager | `cham3-sessiond`, KWin, launcher identity state | User/session-bound `alias_id`, `tag_id`, opaque `window_id`, and `sandbox_id`; verified metadata for navigation. Validate window lifetime/ownership and reject forged or stale targets. No policy mutation. |
| Hot view / inspector / active monitor | KWin attribution, `cham3-sessiond`, `cham3-contractd`, `cham3-policyd`, `cham3-registryd`, runtime/brokers | Resolve actual managed window and current contract validity separately from backend enforcement. Read-only inspection cannot sign. Resource-specific revocation requires enforcement acknowledgement. |
| ECR Explorer | Restricted registry/history interface under approved ADR 0001, once implemented | Authorized read-only history; no app access to stores, administrative methods, signing, or reset/replay authority. |
| Editor / settings | `cham3-policyd`, contract/registry services, `cham3-authui`, `cham3-vaultd`, `cham3-netd` | Validate exact proposed settings and authenticated approval, bind signed content and revision to the affected scope, then apply and confirm. Cancel/reject cannot increase effective authority. |
| Kill switch / status | `cham3-netd`, launcher/supervisor and restricted device-control integration | Authenticated deny request and separately authorized restore; state/confirmation for each relevant domain. Unknown or partial enforcement cannot report fully isolated or restored. |
| HUD previews / input | Trusted KWin integration | Session-scoped compositor rendering and physical input attribution; no application preview access or synthesized authorization. |

State updates must identify the verified user/session, sandbox, contract revision or approved configuration revision, and enforcement domain. Require freshness/ordering and reject stale confirmations, including after runtime-ID reuse, service restart, or rapid context changes. Only confirmed state can drive a compliant pip/badge. Treat titles, icons, application labels, urgency, and supplied rationale as untrusted presentation data; escape and bound them.

### Implementation readiness and scope limits

Before assigning M5-001, freeze the alias/profile isolation mapping, tag/KWin integration, authenticated IPC schema and freshness model, utility-window scoping, network configuration authorization/recovery, global kill persistence/boot/radio behavior, and supported display/overflow matrix. Resolve the protected-preview boundary and shortcut conflicts. ECR implementation remains conditional on its own approved decisions.

Owner choices still required include multi-output placement, minimum supported screen height and overflow behavior, and the exact global restore authentication policy. Select an implementation width within the approved range when those display constraints are settled. Any outcome that changes this approved layout or weakens mandatory policy requires a scoped owner amendment. This documentation task does not implement these dependencies, modify `PROJECT.md`, renumber milestones, or approve unrelated ADRs.

## Alternatives considered

- **Single monolithic privileged panel:** Rejected; combines desktop presentation, signing, and enforcement privileges and enlarges the trusted boundary.
- **Themes/widgets with UI-only network colors and directory separation:** Rejected; does not establish application isolation or confirmed policy enforcement.
- **VM identity wheel:** Rejected; aliases are isolated application execution contexts under existing cham3 services, consistent with the project's non-goals.
- **Stock icons-only tasks alone:** Insufficient for alias/tag scoping, verified network pips, background-tag pulls, and contract actions. Reuse upstream facilities where they satisfy the approved behavior.
- **Radios-only kill switch or external-only blocking:** Rejected; misses wired, namespace, LAN, and loopback paths. Dropping all IPC is also incompatible with preserved authorized `AF_UNIX` desktop control.

## Tradeoffs and consequences

- A compact panel makes security state consistently reachable but constrains labels, hit targets, task overflow, scaling, and accessibility. Validate the supported display matrix before shipping.
- Fifteen view contexts and independent profiles improve organization and reduce accidental profile reuse, while adding persistence, attribution, and lifecycle complexity. They provide no anonymity guarantee.
- Full Isolation Kill also stops IP-local services, including TCP-based local development and gateway connections. Authorized Unix-socket desktop control remains available; IP-dependent failures must be visible.
- Modular packaging reduces privilege coupling but requires versioned service contracts and coherent status reconciliation. Unsupported backend/API combinations expose unavailable/error states rather than optimistic success.
- Upgrades must preserve deliberate user customization, migrate versioned layout/context state, and reconcile against current verified runtime state. Recovery cannot import stale contracts or restore network permission from old UI settings.
- M1 may deliver visual foundations, but unavailable security controls must be marked as unavailable. Full M5 acceptance depends on real backend/VM evidence and the relevant M6 reliability gates.

## Security implications

- Preserve per-application and cross-user isolation, inherited child-process restrictions, default-deny launch, mandatory-policy precedence, no arbitrary vault access, immutable authorized revisions, and enforceable revocation (SEC-01–07, SEC-10, SEC-12, SEC-14–16).
- Keep Direct/Tor/Offline per-application enforcement authoritative; aliases, tags, signed settings, and desired switch positions cannot prove active enforcement. Tor has no Direct fallback; transitions quarantine and restart as required (SEC-08–09, SEC-11, SEC-13).
- Treat trusted input, window attribution, status freshness, configuration validation, Unix-socket proxy denial, and global network coverage as security boundaries requiring adversarial tests. An unlocked vault or application-supplied identity cannot substitute for approval.
- Claims remain bounded by `PROJECT.md` §3.4: compromised kernel/compositor/privileged service, traffic correlation, and information already disclosed are outside asserted guarantees (SEC-18).
- Independent architecture/security review is required for this documentation change and for exact implementation commits and enforcement evidence. Static documentation checks prove neither isolation nor networking behavior.

## Approval and supersession

- **Owner approval evidence:** The project owner's 2026-10-11 repository request states, “We have finalized the architectural and UX design” and directs Codex to “formalize this into an approved Architecture Decision Record,” followed by the nine-component design recorded here. Approval covers that layout and its operation within existing security invariants; it does not approve unresolved backend schemas or certify implementation.
- **Supersedes / is superseded by:** None. Refines `PROJECT.md` §8.2 within its right-side panel scope; preserves §8.3 inspection and §7 network semantics. Does not supersede or approve ADR 0001.
- **Follow-up owner/coordinator action:** Resolve the readiness decisions listed above, assign implementation/review/test owners for M5-001, and admit code only through the repository's independent review and verification gates.
