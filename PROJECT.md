cham3 — Project Specification

Project Name: cham3
Project Type: Debian-based security-focused operating system
Base Distribution: Debian GNU/Linux 13 (Trixie)
Desktop Environment: KDE Plasma 6 on Wayland
Primary Architecture: x86-64 (AMD64)
Project Status: Architecture and initial implementation planning
License: To be selected before public release, with all upstream licensing obligations preserved.

---

1. Project Overview

cham3 is a Debian-based operating system designed around cryptographically authorized application permissions, strict process isolation, and enforced per-application networking.

Its defining principle is:

Applications receive only the authority explicitly granted to them, regardless of which human user launches them or whether the application itself is considered trustworthy.

Conventional desktop operating systems commonly allow applications running under the same user account to share broad access to that user's files, network connections, desktop services, and processes.

cham3 replaces this assumption with application-specific authorization.

Each application is assigned an independently identifiable security context, a signed permission contract, and an isolated execution environment.

The operating system enforces the contract through Linux security mechanisms and trusted resource brokers.

The user controls these contracts through a customized KDE Plasma desktop, including a signature feature: holding the Super key and clicking an application's window opens its contract inspector.

The user may review permissions, change network modes, revoke capabilities, and authorize new grants directly from this interface.

cham3 must run existing Linux applications wherever feasible without requiring developers to adopt a new programming interface.

1.1 Primary objectives

cham3 must deliver:

1. A complete, installable Debian-based operating system.
2. A customized KDE Plasma desktop.
3. Cryptographically signed, tamper-evident application permission contracts.
4. Independent cryptographic signing identities for each human user.
5. Protected private-key storage and restricted signing operations.
6. Isolation between applications belonging to the same user.
7. Three enforceable application network modes: Direct, Tor, and Offline.
8. Fail-closed Tor routing with no automatic clearnet fallback.
9. Runtime inspection and modification of application contracts.
10. Super+Click contract access through KDE.
11. Explicit user consent for expanded permissions.
12. Time-limited, session-limited, one-shot, and persistent grants.
13. Revocation of permissions, including active resource access where technically enforceable.
14. Compatibility with Debian packages, Flatpak applications, and standalone Linux executables.
15. A signed cham3 APT repository and reproducible distribution-building infrastructure.
16. A security-testing framework that verifies actual enforcement rather than merely checking configuration files.

1.2 Non-goals

The initial architecture does not include:

- A separate virtual machine for every application.
- Six fixed virtual identities or six mandatory desktop environments.
- A custom Linux kernel or a complete kernel rewrite.
- Blockchain-backed permissions or cryptocurrency infrastructure.
- Executable blockchain-style smart contracts.
- A requirement for external cryptographic devices.
- A requirement for specialized CPU features.
- A custom cryptographic algorithm.
- Handwritten signatures as private-key generation or authentication mechanisms.
- Claims of guaranteed anonymity against all adversaries.
- Claims that a software-only private key is inaccessible to a fully compromised kernel.
- Automatic compatibility with every Linux application under maximum isolation.

Hardware-assisted signing and stronger virtualization may be considered in future work, but neither is part of the required foundational architecture.

---

2. Foundational Architecture

cham3 consists of five major security and operating-system layers.

2.1 Debian foundation

Debian provides:

- The Linux kernel.
- Hardware drivers.
- Process scheduling and memory management.
- Filesystem infrastructure.
- Network protocol implementations.
- systemd and system services.
- APT and Debian packaging.
- KDE Plasma and its dependencies.
- Application compatibility.

cham3 must remain compatible with Debian package management and normal system-update mechanisms.

The project must not maintain a complete fork of Debian unless a demonstrable requirement cannot be satisfied through packages, configuration, existing kernel facilities, or narrowly scoped upstreamable changes.

2.2 Application isolation layer

The isolation layer launches applications with distinct security identities and restricted access to system resources.

An application's Unix user ID is not sufficient to define its authority.

Two applications running under the same Unix user must still be subject to different security policies.

The isolation layer must control filesystem visibility, process access, networking, IPC, system-service access, and privileged operations.

2.3 Contract authorization layer

The contract layer determines whether a resource request is permitted.

Every application receives an effective permission policy derived from:

- Mandatory system security rules.
- The owning user's authority.
- The application's verified identity.
- Its active signed contracts.
- Its current execution session.
- Applicable expiration and revocation state.

A cryptographic signature proves that a contract was signed by a recognized authority and that the signed contents have not changed.

It does not independently prove that the requested operation is currently authorized.

2.4 Resource enforcement layer

Linux and privileged cham3 resource brokers enforce contract decisions.

Applications do not receive general access to the contract registry or signing vault.

Instead, approved permissions are translated into enforceable kernel restrictions, restricted resource handles, and broker policies.

Resource access must fail when the required authorization is absent.

2.5 KDE desktop integration

The user manages the system through a customized Plasma environment.

The graphical interface provides:

- Network-mode indicators.
- Contract inspection and editing.
- Cryptographic authorization prompts.
- Permission history.
- Revocation controls.
- Application security status.
- Clear separation between requested, signed, and actively enforced policies.

The interface must reflect actual backend enforcement state rather than simply displaying the user's desired configuration.

---

3. Security Model

3.1 Trust boundaries

The system distinguishes four principal types.

Human users

Human users are the owners of cryptographic authorization identities and personal resources.

Each user has independent signing authorities, credentials, permissions, and private storage.

Applications

Applications are untrusted by default.

They may request additional authority but cannot grant themselves permissions or issue contracts in another user's name.

Trusted system services

Trusted services perform privileged authorization, signing, sandbox creation, resource brokering, and enforcement management.

Each service must have the smallest practical set of privileges.

Operating-system kernel

The Debian Linux kernel remains part of the trusted computing base.

cham3 depends on it to enforce memory isolation, process restrictions, networking policy, and access-control decisions.

A complete kernel compromise falls outside the baseline guarantees.

3.2 Default-deny model

An application's baseline environment must provide only what is required to launch and perform explicitly provisioned operations.

It must not automatically receive:

- General access to the user's home directory.
- Other applications' private storage.
- Access to arbitrary processes.
- Unrestricted IPC.
- Microphone or camera access.
- Screen capture capabilities.
- Privileged device interfaces.
- Arbitrary network connectivity.
- Access to cham3 signing keys.
- Permission to change its own security policy.

An application without an approved network policy must start Offline.

Essential runtime resources, application-private storage, and explicitly approved desktop integration interfaces may be provisioned under narrowly defined system policies.

3.3 Authorization hierarchy

The effective permissions of an application are the intersection of all relevant security constraints.

The hierarchy is:

1. Mandatory platform security restrictions.
2. Resource ownership and system administrative policy.
3. Application sandbox restrictions.
4. Active user-authorized contract grants.
5. Runtime capability restrictions.

A signed contract cannot override a higher-level prohibition.

For example, a Tor-only sandbox cannot gain direct internet access merely because an application requests it.

3.4 Threat model

The baseline system is designed to resist:

- Malicious unprivileged applications.
- Applications accessing other applications' resources.
- Unauthorized networking and proxy bypass attempts.
- Forged and modified permission contracts.
- Cross-user authorization attempts.
- Unauthorized use of contract-signing interfaces.
- Permission escalation through ordinary process creation.
- Unauthorized access through IPC or host services.
- Common accidental DNS, IPv4, and IPv6 networking leaks.
- Theft of encrypted private-key files while their keys remain locked.

The initial system does not guarantee protection against:

- A fully compromised kernel.
- A compromised privileged signing or enforcement service.
- Physical attacks that defeat the underlying computer's hardware security.
- Extraction of secrets during software-only signing by a sufficiently privileged attacker.
- All forms of traffic correlation.
- Fingerprinting performed by arbitrary native applications.
- Information that an authorized application has already copied or transmitted.

Every security claim must identify its assumptions and be supported by tests.

---

4. Cryptographic Identity System

Every human user has an independent cryptographic identity.

Applications do not possess the user's private contract-signing keys.

The identity architecture must separate account identity, persistent authorization, and temporary authorization.

4.1 Key hierarchy

Each user is assigned three types of signing authority.

Identity and recovery authority

Used for:

- Certifying contract-signing public keys.
- Rotating signing authorities.
- Recovering account authorization after credential or key loss.
- Invalidating compromised signing authorities.

This key must remain locked during ordinary application use.

Persistent contract authority

Used to authorize long-lived, revocable application grants.

Persistent contracts remain valid until revoked, expired by an additional policy, or invalidated by a change in signing authority or policy state.

Persistent does not mean irrevocable.

Temporary contract authority

Used for bounded authorizations, including one-shot, session-bound, and time-limited grants.

Temporary signing keys or authorization certificates must specify their permitted lifetime and scope.

The verifier must reject persistent grants signed under temporary-only authority.

Temporary authority may be session-specific and rotated when a user session ends.

4.2 Cryptographic primitives

The initial cryptographic design must use:

Function| Selected approach
Digital signatures| Ed25519
Signed contract envelope| COSE_Sign1
Contract serialization| Deterministic CBOR
Private-key encryption| XChaCha20-Poly1305
Passphrase-based key derivation| Argon2id
Hashing| SHA-256
Randomness| Linux cryptographically secure random-number generator
Key identification| Authenticated registry identifier bound to public-key material

Domain separation must distinguish OS contract signatures from signatures used for other purposes.

All cryptographic operations must use maintained, established libraries.

No custom encryption, signing, or password-hashing primitive may be introduced.

OpenPGP is not required for internal contracts.

4.3 Private-key storage

Private signing keys must be encrypted at rest.

Each encrypted key object must contain sufficient authenticated metadata to identify:

- Key type.
- Algorithm.
- Format version.
- Owner.
- Encryption parameters.
- Unique salt.
- Nonce.
- Ciphertext.
- Authentication tag.

Keys must be encrypted with key-encryption material protected through the user's authentication credentials.

Private-key plaintext must never be stored in an ordinary configuration file, log, database field, or application-accessible directory.

The public key may be available for verification.

4.4 Secure signing vault

A dedicated signing service, provisionally named "cham3-vaultd", manages private contract-signing keys.

Its responsibilities are:

- Encrypted key storage.
- Key creation.
- Key unlocking.
- Authorized signing operations.
- Key rotation.
- Key invalidation.
- Secure memory handling.
- Key lifecycle auditing.

The service must not expose an unrestricted signing API to applications.

Untrusted applications may request capabilities through the contract system but must not directly ask the vault to sign arbitrary messages.

The vault must accept signing operations only through an authenticated, validated authorization workflow.

4.5 Memory handling

Private keys must remain encrypted while locked.

During a software-only signing operation, necessary secret material may temporarily exist in protected process memory.

The vault must minimize this period and apply:

- Process isolation.
- Restrictions on process tracing and debugging.
- Memory locking where supported and successfully applied.
- Exclusion of secret buffers from normal core dumps.
- Guarded memory allocations.
- Explicit clearing of sensitive buffers.
- Short-lived signing operations.
- Restrictions on swapping and hibernation exposure.
- No secret logging.

These mechanisms reduce exposure but do not guarantee protection against a fully compromised kernel.

cham3 must not claim that software-only private keys never enter host RAM.

4.6 Contract signing procedure

When the user approves a new or modified contract:

1. The Contract Manager constructs the proposed contract.
2. The contract is validated against system policy.
3. The trusted authorization interface displays the exact security-relevant contents.
4. The user authenticates as required for the requested grant.
5. The authorized contract is encoded into its canonical signed representation.
6. The signing vault signs the approved representation.
7. The signature and contract are submitted to the Contract Registry.
8. The contract is verified before any authority is activated.
9. The enforcement layer applies the authorized changes.
10. The system records the resulting runtime status.

A valid signature must never be interpreted as proof that a permission has already been enforced.

4.7 Key recovery

The system must distinguish identity recovery from identity replacement.

If a user loses their signing credentials, recovery may occur only through a separately established recovery mechanism.

Recovery material must be encrypted and handled independently of normal application grants.

A new key must not automatically be treated as the old identity.

Public-key transitions must be authenticated by an approved recovery or key-rotation authority.

Compromised keys must be revocable, and contracts depending on those keys must be reevaluated.

---

5. Contract Specification

A contract is an authenticated authorization statement that grants a specific application narrowly scoped access to resources.

Contracts must use a fixed, versioned schema.

They must not contain executable scripts or arbitrary programmable logic.

5.1 Required contract fields

Every contract must contain:

Contract identification

- Schema version.
- Unique grant identifier.
- Contract type.
- Policy version.
- Creation timestamp.

Issuer

- Human user identifier.
- Signing-key identifier.
- Signing authority class.
- Applicable authorization-session reference.

Subject

- Owning user identifier.
- Stable application identifier.
- Installation identifier.
- Verified application package or executable identity.
- Runtime scope where applicable.

Resource

- Resource category.
- Stable resource identifier.
- Relevant ownership information.
- Requested operation set.

Validity

- Effective start.
- Expiration where applicable.
- Session binding where applicable.
- Usage restrictions where applicable.

Delegation

- Whether delegation is permitted.
- Maximum delegation depth.
- Maximum delegable scope.
- Parent grant reference where applicable.

Authorization

- Unique authorization request identifier.
- Policy epoch.
- Applicable security profile.
- Relevant mandatory constraints.

The signed representation must be unambiguous and deterministic.

5.2 Signature verification

A contract is cryptographically valid only if:

1. Its format is recognized and permitted.
2. The encoded message is well formed.
3. The signature verifies against the registered issuer public key.
4. The public key is independently authenticated.
5. The signing algorithm is permitted for that contract version.
6. The key's authority class permits the requested grant.

A contract is currently authorized only if it also passes:

- User ownership checks.
- Application identity checks.
- Resource ownership checks.
- Mandatory policy checks.
- Expiration checks.
- Session-binding checks.
- Revocation checks.
- Delegation checks.
- Policy-epoch checks.

Both verification stages are mandatory.

5.3 Contract lifetimes

Four grant lifetimes are required.

One-shot

Authorizes exactly one defined operation.

The grant is consumed after successful use or invalidated according to its failure-handling policy.

Session

Valid only within a specific authenticated user or application session.

Session termination invalidates the grant.

Timed

Valid until a specified expiration.

The enforcement system must stop further covered operations after expiration.

Persistent

Remains valid across sessions until revoked or invalidated.

Persistent grants are always revocable.

5.4 Contract revision

Approved contracts must be immutable.

Editing a contract creates a new signed revision rather than modifying previously signed bytes.

Each revision must identify the preceding authorized state or a versioned contract lineage.

The registry must prevent old, superseded contracts from silently restoring broader permissions.

The authoritative registry state determines which revisions remain active.

5.5 Revocation

Revocation must invalidate subsequent authorized use.

The system must maintain authoritative state identifying revoked grants and related descendants.

Revocation must be applied to resource brokers and active sandbox enforcement.

Open file descriptors, memory mappings, existing sockets, and in-flight operations require resource-specific handling.

The system must not imply that revocation can erase data previously read or transmitted.

For resources requiring strict immediate revocation, access must be mediated through mechanisms capable of actively terminating or refusing subsequent operations.

5.6 Delegation

Delegation is denied by default.

Where authorized, a child grant must never contain more authority than its parent.

Delegated grants may reduce permissions, duration, resource scope, and delegation depth.

They must not expand them.

Revoking a parent grant must invalidate dependent grants unless those grants have separate independent authorization.

---

5.7 Eternal Contract Repository (ECR) — proposed adoption amendment

Pending scoped owner approval of ADR 0001, the following becomes the required contract registry architecture:

The ECR is an append-only signed contract DAG owned by cham3-registryd, partitioned by verified user and application/installation identity. Persistent history and bounded temporary branches retain immutable lineage; historical objects do not retain expired, consumed, revoked, or superseded authority. Sandboxed applications have no direct access to its store, history indexes, administrative interfaces, or signing operations.

User-authorized contract additions, modifications, explicit revocations, merges, and forward restores require durable signed transactions binding the exact state, delta, identity, lifetime, policy epoch, and parents. No new or expanded authority may activate before verified durable admission and confirmed resource enforcement. Restore creates a new current-parented revision under current policy and fresh approval; old signatures or head resets cannot reactivate authority.

Mandatory expiration, session termination, one-shot consumption, key/policy invalidation, emergency restriction, and fail-closed denial must occur immediately, even when a signed transaction or lifecycle audit write is unavailable. An explicit revocation whose commit fails still requires immediate denial/quarantine. Trusted lifecycle evidence cannot authorize or restore grants, impersonate human approval, or permit background user-key signing. Audit-write failures are reported as gaps while access remains denied or unresolved; recovery cannot presume valid authority or complete history.

ADR 0001 defines the scoped architecture and M2/M5/M6 responsibilities. Detailed schema, storage, lifecycle evidence, retention/privacy, authentication/identity transitions, and rollback-resistant recovery designs require independent review and owner approval before affected implementation. Architecture approval alone is not runtime security evidence.

---

6. Application Isolation

cham3 must be compatible with existing Linux applications while enforcing application-specific security boundaries.

6.1 Application identity

Every launched application must have an authenticated runtime identity.

The system must not rely solely on application display names, window titles, or caller-provided process identifiers.

Application identity must bind together the owning user, application package or executable, installation instance, and runtime process context.

Executable replacements and application updates must follow an explicit identity transition policy.

Applications cannot acquire another application's grants by copying its name or launching its executable.

6.2 Linux isolation mechanisms

The initial implementation must use existing Debian-supported mechanisms:

Mechanism| Responsibility
Linux namespaces| Isolate network, mount, IPC, process and other views as appropriate
AppArmor| Mandatory application and service restrictions
Seccomp| Restrict unnecessary system calls
cgroups v2| Process grouping and resource limits
systemd| Managed service and application lifecycles
Bubblewrap| Sandboxed userspace environments where appropriate
Flatpak| Existing application sandboxing infrastructure
XDG Desktop Portals| Controlled desktop-resource access
nftables| Enforced network traffic policies

Isolation must be established before launching untrusted executable code.

The application must not be permitted to escape its security context through privileged namespace operations, unrestricted helper processes, or unauthorized system services.

6.3 Child processes

Child processes must inherit restrictions without acquiring additional authority.

Process creation must not implicitly authorize broader filesystem access, networking, IPC, or device operations.

An executable transition must not allow the caller to impersonate a different authorized application.

Applications spawning helpers must remain inside the relevant contract and sandbox enforcement boundaries unless a separately authorized transition occurs.

6.4 Existing application support

Three application classes are required.

Native Debian applications

Applications installed through APT must be launchable through the cham3 sandbox manager when compatible.

The launcher must provision the necessary runtime libraries, desktop integration, and application-private storage without exposing unrelated user resources.

Flatpak applications

The system must integrate with Flatpak's existing sandbox and portal architecture.

cham3 contracts must remain authoritative for networking and additional protected resources.

Existing Flatpak permissions must not override more restrictive cham3 policies.

Standalone executables

Standalone ELF binaries, AppImages, and comparable programs must be supported through the sandbox launcher where practical.

Programs that cannot operate within the required restrictions must report their incompatibility.

cham3 must not silently disable isolation to make them work.

6.5 Privileged and system applications

Core system services require separate administrative security policies.

They must not be treated as ordinary desktop applications with unrestricted user permissions.

Trusted services must be explicitly identified, reviewed, restricted, and audited.

Administrative privilege must not automatically grant permission to issue user-authenticated contracts through the normal contract workflow.

The baseline system still trusts privileged kernel execution and therefore cannot prevent every form of abuse following a kernel-level compromise.

---

7. Per-Application Networking

This is a defining cham3 subsystem.

Every ordinary network-capable application must be assigned exactly one mandatory network mode:

- Direct.
- Tor.
- Offline.

The mode is a property of the application's active security contract and its enforced sandbox state.

Applications may not independently change their network mode.

7.1 Direct mode

Visual identity: Rose.

Direct permits ordinary internet networking through a controlled host routing and firewall environment.

The application may use networking protocols permitted by its additional restrictions.

Direct does not provide an anonymity guarantee.

Direct applications must still be subject to application-specific process isolation and applicable network destination restrictions.

7.2 Tor mode

Visual identity: Purple.

Tor mode routes supported external communication through a managed Tor networking subsystem.

The application must not have a direct clearnet route.

Required properties:

- All permitted external TCP traffic is routed through the approved Tor gateway.
- DNS resolution cannot bypass the gateway.
- Unsupported networking protocols are denied.
- Direct IPv4 and IPv6 escape paths are prohibited.
- Raw sockets and privileged network operations are unavailable.
- Local-network access is denied unless specifically handled by a separate trusted policy that cannot compromise Tor-only egress.
- Tor disconnection causes traffic to fail closed.
- No automatic fallback to Direct is permitted.

The system must not claim that arbitrary native applications become anonymous merely because their traffic passes through Tor.

Application-specific identifiers, protocol behavior, browser fingerprints, login credentials, and network timing can still permit correlation.

Tor Browser should be the recommended application for anonymous web browsing.

7.3 Offline mode

Visual identity: White.

Offline provides no external network access.

An Offline application must not receive a network route capable of reaching the internet or local network.

Access to loopback and local IPC must be restricted so that another service cannot transparently act as a network proxy.

Offline does not necessarily mean that the application has no local communication at all; authorized IPC may remain available under explicit policy.

The distinction must be visible in the application contract.

7.4 Network namespaces

Every isolated application instance must execute within a managed network namespace or an equivalent enforceable sandbox network context.

The networking manager must control:

- Network interfaces.
- Routes.
- DNS configuration.
- IPv4 and IPv6 connectivity.
- Firewall policy.
- Network namespace lifecycle.
- Application process membership.
- Connection teardown.
- Resource accounting.

An application must not be able to join the host's unrestricted network namespace.

7.5 Tor gateway

The Tor backend must be managed as a dedicated privileged networking component.

It must provide controlled routing for application namespaces without exposing unrestricted gateway administration to applications.

The initial implementation must support transparent TCP routing and managed DNS resolution where feasible.

Protocols that cannot be safely supported through Tor must fail rather than escape through another connection.

Tor stream isolation should separate unrelated application contexts.

Tor circuits must not be presented as guaranteed unlinkable identities.

7.6 Firewall enforcement

Network policy must be enforced through host-controlled nftables rules and namespace configuration.

Policies must be established before the application gains access to the network.

A failure to apply the required policy must prevent the application from launching with external connectivity.

The system must test both IPv4 and IPv6.

Firewall reloads, network changes, Tor restarts, suspend/resume, and application crashes must not create temporary Direct fallback routes for Tor or Offline applications.

7.7 Network-mode transitions

Changing an application's network mode is a security-sensitive lifecycle operation.

The implementation must distinguish proposed policy from currently enforced policy.

For the initial release, switching between Direct, Tor, and Offline requires a controlled application restart.

The sequence is:

1. The user proposes the new mode.
2. The contract system checks the request.
3. Additional user authorization is obtained when required.
4. The modified contract is signed and verified.
5. The application is placed under a deny-egress quarantine.
6. Existing network connections are terminated.
7. The application and its managed process tree are stopped.
8. The new network environment is constructed and verified.
9. The application restarts in the new environment.
10. The registry records successful activation of the new policy.

If any enforcement step fails, the application must remain without external network access.

The system must not indicate that the new contract is enforced until the sandbox manager confirms it.

7.8 Transition requirements

Transition| Required behavior
Direct → Offline| Immediate network quarantine followed by controlled restart
Direct → Tor| Quarantine, connection teardown and restart inside Tor environment
Tor → Offline| Immediate network quarantine followed by controlled restart
Tor → Direct| Explicit authenticated approval, security warning and controlled restart
Offline → Tor| Authorized contract change and controlled launch
Offline → Direct| Authorized contract change and controlled launch

Tor-to-Direct transitions must not silently reuse state intended to remain anonymous.

The initial implementation should use a separate application data profile when changing between Tor and Direct contexts if the existing profile contains identity-linkable state.

The system must not imply that restarting an application automatically removes cookies, account identifiers, caches, or other identifying data.

7.9 Application network indicators

KDE must show each application's actual enforced mode.

The indicator colors are:

- Purple: Tor.
- Rose: Direct.
- White: Offline.

The system must separately report pending changes and enforcement failures.

An application whose intended mode differs from its actual enforced state must never be represented as fully compliant.

---

8. KDE Plasma Desktop

cham3 must ship with a customized KDE Plasma 6 desktop.

The base environment is KDE Plasma on Wayland.

The project should extend upstream KDE using themes, widgets, Qt/Kirigami applications, and narrowly scoped KWin integrations.

It should avoid maintaining a complete KDE fork unless a specific required feature cannot otherwise be implemented.

8.1 Visual design

The desktop should follow the cham3 identity:

- Dark primary surfaces.
- Minimal, geometric controls.
- Solid rather than heavily blurred or gradient-dependent status indicators.
- Purple for Tor.
- Rose for Direct.
- White or neutral gray for Offline.
- Clear application state and security information.
- Consistent icons and restrained visual effects.

The desktop must prioritize usability and trustworthy security information over decorative complexity.

8.2 Right-side taskbar

cham3 must provide a vertical taskbar on the right side of the screen as its default desktop arrangement.

The taskbar must support:

- Running application indicators.
- Network-mode indicators.
- Application switching.
- Contract inspection access.
- Standard KDE system tray capabilities.
- Clock and session controls.
- Security and enforcement status.

Unlike the discarded multi-VM design, the taskbar does not require an identity wheel for switching between virtual machines.

The three colors represent application network policy.

8.3 Super+Click contract inspection

Holding Super and left-clicking an application window must open that application's contract inspector.

This is a foundational desktop interaction.

The system must:

1. Detect the modifier and click through KWin's trusted input path.
2. Identify the actual managed window.
3. Resolve the window to its verified application and runtime sandbox.
4. Retrieve the associated contract.
5. Display the contract inspector for that application.
6. Allow inspection without requiring a signing operation.
7. Require authorization before modifying security-sensitive permissions.

Window titles and application-provided labels cannot be trusted as proof of application identity.

The inspector must display an authenticated application identity established by the sandbox and window-management infrastructure.

KDE shortcut conflicts must be resolved through the cham3 desktop configuration.

An alternative keyboard-only shortcut and taskbar/context-menu action must provide equivalent access.

8.4 Contract inspector

The contract inspector must provide:

Application identity

- Application name.
- Verified application identifier.
- Publisher or package origin where available.
- Owning human user.
- Running process/sandbox identity.
- Contract verification status.

Network policy

- Direct.
- Tor.
- Offline.
- Currently enforced mode.
- Pending mode changes.
- Required restart status.

Resource permissions

- Filesystem access.
- Microphone.
- Camera.
- Clipboard.
- Screen capture.
- Relevant IPC.
- Other resource permissions supported by cham3.

Contract validity

- Grant lifetime.
- Expiration.
- Signing authority.
- Revocation state.
- Policy version.
- Last successful enforcement status.

Actions

- Propose new permissions.
- Revoke existing permissions.
- Change network mode.
- Change grant duration.
- Review prior contract revisions.
- Inspect enforcement failures.
- Restart the application under an updated policy.

The user must be able to distinguish an editable proposal from an already authorized and enforced contract.

8.5 Trusted authorization interface

Security-sensitive approval dialogs must be controlled by trusted desktop and authorization components.

Applications must not be able to impersonate system approval prompts.

Ordinary applications must not be permitted to synthesize input that authorizes contracts.

Accessibility and automation capabilities require separate restricted treatment.

A fully compromised compositor or kernel remains outside the baseline protection guarantee.

8.6 KDE packaging

The desktop must be delivered as maintainable Debian packages containing:

- Plasma global theme.
- Plasma style and colors.
- Icons.
- Wallpapers.
- Default panel layout.
- KWin integration.
- Contract inspector.
- Permission dashboard.
- Security notifications.
- Relevant System Settings integration.

Regular KDE functionality must remain available unless a specific feature conflicts with the security model.

---

9. Resource Permission Brokers

Network isolation is only one part of the contract system.

cham3 must also mediate sensitive operating-system resources.

9.1 Filesystem broker

Applications should receive access to selected files or directories through restricted handles or approved filesystem views.

The broker must enforce:

- Read permissions.
- Write permissions.
- Creation and deletion where supported.
- Resource identity.
- Grant expiration.
- Revocation.
- Ownership.
- Application binding.

Approved file resources must be identified in a manner resistant to path substitution and symlink-based attacks.

Applications must not automatically receive access to the user's entire home directory.

9.2 Desktop portals

cham3 must integrate with XDG Desktop Portals for resource requests that can be handled through existing Linux desktop APIs.

File selection, screen capture, device access, and other supported operations should use the upstream portal model where practical.

A user selecting a file must not implicitly grant unrelated directory access.

cham3's contract registry must remain authoritative over the permissions represented by portal grants.

9.3 Device access

Sensitive devices must not be available merely because they are reachable through ordinary filesystem or device-node permissions.

Device access requires appropriate mediation.

Initial protected categories include:

- Microphone.
- Camera.
- Screen capture.
- Sensitive input devices.
- Clipboard where mediation is feasible.
- Privileged USB and hardware operations.

The system should use existing PipeWire, portal, device-management, and Linux security mechanisms where suitable.

9.4 IPC protection

Applications must not be able to bypass contracts by requesting that a more privileged service perform a prohibited operation on their behalf.

Privileged brokers must authenticate the originating application context and verify its effective authority.

Descriptor passing, shared memory, Unix sockets, and D-Bus interfaces must be included in the threat model.

Service interfaces must be reviewed for confused-deputy vulnerabilities.

---

10. Multi-User Behavior

cham3 must support multiple human users on one installation.

Each user has independent:

- Cryptographic identities.
- Signing authorities.
- Private-key vaults.
- Contract registries or appropriately isolated registry views.
- Application grants.
- Authentication sessions.
- Private resource ownership.
- Recovery configuration.

An application running under one user must not acquire grants issued to another user.

The same installed application may have different contracts for different users.

A contract must be bound to the correct user and application installation context.

Shared resources require authorization from the appropriate resource owner or administrative policy.

User switching must invalidate or suspend authorization ceremonies that no longer have a trustworthy active session.

Privileged administrative access and user contract-signing authority must remain conceptually separate, subject to the acknowledged limitations of kernel-level privilege.

---

11. Application Lifecycle

Every application launch must pass through the cham3 sandbox lifecycle.

11.1 Launch procedure

The system must:

1. Resolve the executable and installation identity.
2. Determine the owning user.
3. Load relevant contracts and mandatory policies.
4. Validate current authorization.
5. Select the required network mode.
6. Construct the restricted sandbox.
7. Establish approved resource handles and portals.
8. Apply network and resource restrictions.
9. Verify enforcement setup.
10. Start the application.
11. Register the process tree and runtime identity.
12. Expose accurate state to the KDE interface.

An application must not be allowed to execute with broader privileges while its restrictions are still being configured.

11.2 Execution-state tracking

The sandbox manager must track:

- Application instance.
- Process group or cgroup.
- Network namespace.
- Effective contract version.
- Approved capabilities.
- Enforcement status.
- Current network mode.
- Pending changes.
- Restart requirements.

11.3 Contract modification

Contract changes must use a transactional authorization workflow.

A signed contract proposal does not become active until enforcement succeeds.

If changes cannot be applied safely to a running process, the system must require a restart or terminate the affected capability.

The UI must communicate the outcome accurately.

11.4 Application termination

When an application ends, session-bound capabilities must be invalidated.

Managed resource handles must be released.

Ephemeral authorization state must be cleaned up.

Persistent contracts remain in the registry unless revoked.

---

12. Debian Distribution and Packaging

cham3 must be maintained as a Debian derivative, not as a copied Debian source tree.

The project repository contains cham3-specific source code, configuration, packaging, test infrastructure, and image-building definitions.

Debian packages remain upstream dependencies.

12.1 Distribution image

The project must produce a bootable x86-64 installation image containing:

- Debian base system.
- KDE Plasma Wayland.
- cham3 packages.
- Necessary sandbox and networking infrastructure.
- Security policy defaults.
- cham3 theme and desktop layout.
- Installer integration.
- Signed cham3 package repository configuration.

The initial build system should use Debian live-build and related Debian image-building tools.

12.2 Debian packages

Each major service or functional group should be packaged independently where practical.

Required metapackages:

Package| Responsibility
"cham3-base"| Essential services and OS configuration
"cham3-core"| Contracts, identity, policy and signing
"cham3-sandbox"| Sandboxed application execution
"cham3-networking"| Network namespaces, routing and Tor integration
"cham3-desktop"| KDE customization and contract UI
"cham3-security"| AppArmor, firewall and system hardening
"cham3-full"| Complete desktop installation

Package boundaries must minimize unnecessary dependencies and privilege coupling.

12.3 APT repository

cham3 must maintain its own signed APT repository for custom packages.

Repository-signing keys must be separate from user contract-signing keys.

Installed systems must authenticate repository metadata and packages through standard APT trust mechanisms.

Ordinary development and pull-request workflows must not possess production release-signing credentials.

12.4 Updates

The system must support regular Debian security updates and cham3 component updates.

Updates to contract schema, enforcement services, desktop integration, or policies must include migration and rollback planning.

An update must not silently expand application authority.

Changes that invalidate existing contracts must be handled explicitly.

---

13. Repository Structure

The project should begin as a monorepo.

The structure below defines primary ownership of each subsystem.

Root

"cham3/"

Contains the root project documentation, build entry points, development configuration, CI definitions, and source directories.

Build

"build/"

Contains:

- Installation image configuration.
- Debian live-build configuration.
- Installer configuration.
- Build orchestration.
- Package assembly.
- Release-image tooling.
- Development VM testing infrastructure.

Packaging

"packaging/"

Contains:

- Debian packaging definitions.
- Metapackage definitions.
- Signed APT repository configuration.
- Package dependency manifests.
- Application compatibility profiles.

Core

"core/"

Contains:

- "identityd" — Human user cryptographic identities.
- "vaultd" — Protected private-key storage and signing.
- "contractd" — Contract creation, verification and revision management.
- "policyd" — Authorization evaluation.
- "registryd" — Grants, expiration, delegation and revocation.
- "contract-format" — Canonical contract schema.
- Shared libraries for contract operations.

Sandbox

"sandbox/"

Contains:

- Application launcher.
- Application identity resolution.
- Namespace lifecycle.
- AppArmor policy integration.
- Seccomp configuration.
- Process and cgroup tracking.
- Runtime contract enforcement state.

Networking

"networking/"

Contains:

- Network-policy manager.
- Namespace routing manager.
- Direct networking backend.
- Tor gateway backend.
- Offline networking backend.
- DNS mediation.
- nftables policy definitions.
- Network transition management.
- Leak-detection tests.

Desktop

"desktop/"

Contains:

- Plasma global theme.
- Right-side taskbar configuration.
- KWin Super+Click integration.
- Contract inspector.
- Trusted authorization interface.
- Permission dashboard.
- Network-status indicators.
- Security notifications.
- System Settings integrations.

Portals

"portals/"

Contains:

- Filesystem broker integration.
- Device broker integration.
- Clipboard integration.
- IPC policy integration.
- XDG Desktop Portal integration.

Security

"security/"

Contains:

- System AppArmor profiles.
- polkit rules.
- D-Bus security policies.
- nftables rules.
- systemd service hardening.
- Key management specifications.
- Threat models.
- Security invariants.

Tests

"tests/"

Contains:

- Cryptographic verification tests.
- Policy engine tests.
- Network leak tests.
- Application isolation tests.
- Contract modification tests.
- Revocation tests.
- Multi-user tests.
- KDE interaction tests.
- Application compatibility tests.
- Full installation and boot tests.
- Adversarial security tests.

Documentation

"docs/"

Contains:

- Architecture specification.
- Threat model.
- Contract format.
- Cryptographic key management.
- Network isolation design.
- KDE integration.
- Packaging and release procedures.
- Security testing.
- Development roadmap.

---

14. Service Architecture

The primary cham3 services are:

Service| Responsibility
"cham3-identityd"| Registered user identities and signing authorities
"cham3-vaultd"| Encrypted private-key storage and authorized signing
"cham3-contractd"| Contract verification and authorized revisions
"cham3-policyd"| Effective authorization decisions
"cham3-registryd"| Grant state, expiration and revocation
"cham3-launcherd"| Application sandbox creation
"cham3-netd"| Network namespace and routing enforcement
"cham3-sessiond"| Application/window/session attribution
"cham3-portald"| Contract-aware resource brokering
"cham3-authui"| Trusted user authorization flow

The service names are the canonical intended component names, though they may be consolidated into fewer processes if this demonstrably reduces attack surface without weakening isolation.

Services must communicate through authenticated, narrowly scoped interfaces.

Application-provided claims about identity or permission must not be treated as authoritative.

Only trusted services may initiate key-vault signing operations, and each signing operation must correspond to an independently validated user approval.

---

15. Security Testing Requirements

A security feature is considered implemented only when its enforcement has been demonstrated.

Configuration changes alone are insufficient.

15.1 Cryptographic verification

Tests must prove:

- Valid contracts verify correctly.
- Modified contracts fail verification.
- Unknown or substituted signing keys are rejected.
- Temporary authority cannot authorize persistent grants.
- Expired grants are rejected.
- Revoked grants are rejected.
- Superseded contract revisions cannot restore old authority.
- Cross-user contracts cannot be reused.
- Application identities cannot be substituted.
- Malformed contract encodings are rejected.

15.2 Application isolation

Tests must prove:

- Applications cannot read unrelated private files.
- Applications cannot inspect unauthorized processes.
- Child processes do not gain additional permissions.
- Applications cannot access the signing vault directly.
- Applications cannot modify enforcement policy.
- Unauthorized IPC is rejected.
- Resource-broker requests enforce caller identity.

15.3 Network isolation

Tests must prove:

- Offline applications cannot reach external IPv4 or IPv6 destinations.
- Tor applications cannot connect directly to external destinations.
- DNS requests cannot bypass Tor policy.
- UDP and unsupported traffic do not bypass Tor.
- Tor failure prevents external connectivity.
- Firewall reloads do not create temporary leaks.
- Network changes do not introduce Direct fallback.
- A malicious application cannot escape its network namespace.
- Changing modes terminates existing connections.
- Enforcement failures leave applications quarantined.

15.4 Contract editing

Tests must prove:

- Super+Click selects the intended application.
- Window-name spoofing does not redirect the inspector to another app's authority.
- Inspecting a contract does not itself issue authorization.
- Permission changes require the appropriate authentication.
- New revisions invalidate superseded authorization as defined by policy.
- Unenforced contracts are not displayed as active.
- Cancelled changes do not modify effective authority.

15.5 Compatibility

Compatibility tests must cover:

- Native Debian GTK applications.
- Native Debian Qt applications.
- KDE applications.
- Flatpak applications.
- Standalone executables.
- Applications with helper processes.
- Browsers.
- Networked communications applications.
- Applications using desktop portals.

Unsupported functions must fail visibly rather than silently removing security restrictions.

---

16. Development Roadmap

Development must proceed through independently testable milestones.

Phase 1 — Debian Distribution Foundation

Goal: Produce a bootable cham3 development ISO.

Deliverables:

- Debian 13 base image.
- KDE Plasma Wayland.
- cham3 branding.
- Right-side default taskbar.
- Initial Debian packaging.
- Automated image building.
- Development VM boot tests.

Completion condition: The installation image boots successfully into the customized KDE desktop and can install cham3 packages through its configured package sources.

Phase 2 — Cryptographic Contract Core

Goal: Implement authenticated, tamper-evident contracts.

Deliverables:

- Per-user identity registry.
- Encrypted signing vault.
- Ed25519 signatures.
- Deterministic CBOR contract format.
- COSE signed envelopes.
- Contract verification.
- Temporary and persistent signing authorities.
- Eternal Contract Repository: append-only signed contract DAG, canonical authorization transactions, protected current-state indexes, and replay-resistant recovery under ADR 0001 (proposed adoption amendment).
- Expiration and revocation logic, including mandatory denial independent of signing or ECR availability.

Completion condition: Unauthorized changes to a contract invalidate its signature, and users cannot issue contracts under another user's identity through ordinary interfaces. Under the proposed ADR 0001 adoption, Phase 2 also requires independently reviewed ECR admission, immutable lineage, current-state and crash/replay evidence; failed writes cannot grant authority or postpone mandatory denial. Later phases verify actual application isolation, KDE presentation, and runtime resource enforcement.

Phase 3 — Application Sandbox Runtime

Goal: Give existing Debian applications independent security contexts.

Deliverables:

- Application identity manager.
- Sandboxed launcher.
- Namespace management.
- AppArmor integration.
- Seccomp restrictions.
- Process tracking.
- Basic resource broker integration.

Completion condition: Two applications running under the same user can be prevented from accessing each other's protected resources.

Phase 4 — Three Network Modes

Goal: Enforce Direct, Tor and Offline on a per-application basis.

Deliverables:

- Network namespace manager.
- Tor gateway.
- Direct networking backend.
- Offline namespace policy.
- DNS restrictions.
- nftables enforcement.
- Fail-closed behavior.
- Automated leak tests.

Completion condition: Applications cannot bypass their assigned networking mode through ordinary system calls or configuration changes.

Phase 5 — KDE Contract Integration

Goal: Make contracts manageable from the desktop.

Deliverables:

- Super+Click interception.
- Verified window-to-application attribution.
- Contract inspector.
- Network-mode indicators.
- Contract modification workflow.
- Trusted approval UI.
- Signed contract revisions.

Completion condition: The user can open a running application's contract, propose a change, authorize it, and observe its verified enforcement status.

Phase 6 — Safe Live Policy Management

Goal: Safely handle modifications to running applications.

Deliverables:

- Application network quarantine.
- Connection termination.
- Controlled process restart.
- Runtime enforcement confirmation.
- Failure rollback to denied connectivity.
- Active capability revocation.

Completion condition: Switching an application between network modes cannot create an unauthorized connectivity window.

Phase 7 — Desktop Application Compatibility

Goal: Support mainstream Linux desktop software.

Deliverables:

- Native Debian application profiles.
- Flatpak integration.
- Standalone executable support.
- Filesystem portals.
- Device permissions.
- IPC restrictions.
- Improved desktop integration.

Completion condition: A defined compatibility test suite passes without bypassing mandatory cham3 policy.

Phase 8 — Multi-User Security and Hardening

Goal: Make the system suitable for real multi-user installations.

Deliverables:

- Independent user vaults.
- User-specific grants.
- Shared-resource authorization.
- Session isolation.
- Key recovery and rotation.
- Privileged-service hardening.
- Security auditing.
- Recovery from interrupted contract changes.

Completion condition: Cross-user access and signing attempts are rejected, and authorization state remains consistent across crashes, logouts, and restarts.

Phase 9 — Public Release Preparation

Goal: Produce a testable, maintainable operating-system release.

Deliverables:

- Installer and recovery environment.
- Signed APT repository.
- Secure updates.
- Documentation.
- Reproducible build infrastructure where feasible.
- Regression testing.
- Published threat model.
- Known-limitations documentation.
- External security review preparation.

Completion condition: The system can be installed, updated, recovered, and evaluated by users outside the development environment.

---

17. First End-to-End Demonstration

The initial complete security demonstration must use existing Debian software.

Firefox and Kate are recommended test applications.

The setup must establish:

- Firefox running under a Tor contract.
- Kate running under an Offline contract.
- Both applications owned by the same human user.
- Independent application security contexts.
- A functioning per-user contract-signing identity.

The test must demonstrate the following sequence.

1. Firefox launches with only Tor-routed external access.
2. Attempts to establish unauthorized direct connections fail.
3. Kate launches with no external networking.
4. Kate cannot connect to the internet even when deliberately instructed to do so.
5. The user holds Super and clicks the Kate window.
6. The trusted contract inspector opens Kate's verified contract.
7. The user changes the proposed network mode from Offline to Direct.
8. The system obtains the required authenticated approval.
9. A new contract revision is cryptographically signed.
10. The new signature verifies successfully.
11. Kate remains under its previous enforced restriction until a safe transition.
12. The sandbox manager quarantines and restarts Kate.
13. Kate launches under the new verified Direct policy.
14. The desktop reports Direct only after enforcement is confirmed.
15. A modified or forged copy of the contract fails verification.
16. The user can revoke the grant and terminate further authorized networking.

This demonstration is the first major system-integration milestone.

---

18. Security Invariants

The following rules must remain true throughout development.

Invariant 1 — No ambient application trust.
Installing or executing an application does not grant unrestricted access to user resources.

Invariant 2 — No application signing authority.
An application cannot independently produce valid OS authorization contracts in a human user's name.

Invariant 3 — No unsigned privilege escalation.
A permission increase requires valid authorization from the appropriate authority.

Invariant 4 — Contracts are immutable.
Modification requires a new authenticated revision.

Invariant 5 — Enforcement is authoritative.
A signed contract does not replace kernel and broker enforcement.

Invariant 6 — Default-deny networking.
Unclassified applications do not receive unrestricted internet access.

Invariant 7 — No Tor fallback.
Tor connection failure cannot silently convert an application to Direct networking.

Invariant 8 — Offline means no external networking.
Applications cannot circumvent Offline restrictions through ordinary networking or unauthorized proxy services.

Invariant 9 — Application boundaries survive child processes.
Forking or executing other programs must not grant additional authority.

Invariant 10 — User identities remain separate.
An application or user cannot reuse another user's contract-signing authority.

Invariant 11 — Grant revocation must be enforceable.
Resource classes requiring immediate revocation must use suitable mediated or revocable access mechanisms.

Invariant 12 — Effective policy is never inferred from UI state.
The desktop must display confirmed enforcement status.

Invariant 13 — Network transitions fail closed.
A failed policy change leaves the affected application without unauthorized external access.

Invariant 14 — Cryptographic secrets are never intentionally exposed through ordinary interfaces.
Private signing keys must not be available through application APIs, logs, or unencrypted persistent storage.

Invariant 15 — Security claims must match the threat model.
cham3 must not promise absolute anonymity, protection against a fully compromised kernel, or complete confidentiality of software signing keys during active signing.

---

19. Final Project Definition

cham3 is a Debian-based, KDE Plasma-powered operating system that replaces broad implicit application authority with cryptographically authenticated, application-specific permission contracts.

Each human user controls an independent signing identity. Applications execute within restricted Linux security environments and receive only the permissions authorized by active contracts and mandatory system policy.

Every application operates under one of three enforced networking modes: Direct, Tor or Offline.

The desktop provides an integrated contract-management experience, centered on holding Super and clicking an application window to inspect or modify its permissions.

Cryptographic signatures ensure that contracts cannot be altered without detection, while Linux isolation mechanisms and trusted resource brokers enforce the actual restrictions.

cham3 is designed to support existing Linux software, preserve Debian compatibility, and remain usable without additional security hardware.

The project's defining objective is to give users direct, verifiable control over what every application may do, with mandatory security boundaries that applications cannot voluntarily ignore.