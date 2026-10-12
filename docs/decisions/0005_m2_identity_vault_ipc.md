# ADR 0005: M2 identity, authentication, vault and service IPC (G3)

- **Status:** PROPOSED (revision 3) — awaiting independent re-review and owner approval. Nothing here is approved.
- **Date / decision owner:** Proposed 2026-10-12 by Claude Code (M2 implementer); revised 2026-10-12 after the independent review published at 9cf882503848f8c6cc4938a588cc9fc062a9392d (CHANGES_REQUESTED: R03, R05, R08 size contract; I03 scope); revision 3 on 2026-10-12 after the re-review at 336f13be6a19bb871ea956d64f9cacb0936fe028 (CHANGES_REQUESTED: R13 — the recovery signing that containment requires was itself denied) / cham3 project owner. Finding resolutions: [revision 2](../reviews/M2-001-002-finding-resolutions.md), [revision 3](../reviews/M2-001-002-finding-resolutions-r2.md).
- **Related task/spec sections:** [M2 task](../tasks/M2-cryptographic-contract-core.md) G3, M2-004, M2-007, A04, A05, A12; PROJECT.md §§3.1, 4.1–4.7, 6.1, 10, 14; [ADR 0001](0001_eternal_contract_repository.md) trusted access and approval; SEC-05, SEC-12, SEC-15, SEC-16.
- **Interface specification:** [m2_service_ipc_v1.md](../specs/m2_service_ipc_v1.md). Wire objects: [ADR 0004](0004_m2_wire_schema_hash.md).

`PROJECT.md` is the canonical product specification. This ADR records a scoped decision or proposal and does not silently replace that document.

## Problem and context

M2 needs a trusted identity source for users, keys and (until M3) applications; authenticated key enrollment, rotation, invalidation and recovery; fresh authenticated approval before the M5 trusted UI exists; exact binding of approval to bytes, head, epoch and request; one-use signing; service credentials, roles and protocol; the encrypted key format; KDF limits; and the vault's memory, core-dump, ptrace and locking policy. No arbitrary signing and no trust in bare UIDs or display names.

## Decision (proposed)

### 1. Identity sources

| Identity | Source in M2 | Limits |
|---|---|---|
| Human user | Linux account UID from `SO_PEERCRED` of a `trusted_client` connection (UID in login.defs regular range) bound at first enrollment to the cham3 identity `user_id` (key id of the identity key). | A UID is not authority by itself: every signing operation also needs the user's passphrase. Root is never a signing user. |
| Signing keys | identityd's index of enrollments, certificates, invalidations and replacements signed by the user's identity key, **persisted only in the user's anchored registry journal** (ADR 0006 §1, §5). Caller-supplied key ids are only lookups. | Freshness equals the journal's rollback protection; unknown or contained state denies. |
| Application / installation / executable | **Fixture in M2**: supplied by the trusted client and bound into the signed subject. Real attribution (launcher, package database, executable hashing) is M3. | Labeled fixture; establishes no isolation or attribution claim. |
| Desktop/app session | Session events and liveness from the `fixture_consumer` role standing in for the future cham3-sessiond; unknown liveness denies. | Labeled fixture. |
| Initiating program | Derived by contractd from the caller role (`cham3_dev_cli` for `trusted_client` in M2), never from request text. | |

### 2. Key hierarchy and lifecycle

- Per user: one **identity** key (certifies keys, signs invalidations, never signs commits), one **persistent** contract key and one **temporary** contract key (certificate `not_after` ≤ 7 days, re-certified on rotation). All Ed25519, generated in vaultd from `getrandom(2)` into locked memory.
- Authority rules (identityd layer of verification): `temporary_grant` commits may be signed by temporary or persistent keys; every other commit type requires a persistent key; certificates/invalidations require the user's current identity key; a key's owner must equal the commit's `actor`/`subject`; temporary keys are rejected outside their certificate window; an invalidated key with reason `compromised` makes every commit it signed ineligible (ADR 0007); `rotated`/`retired` keys stop signing but their earlier commits stay verifiable and eligible.
- **Enrollment:** `cham3-ctl enroll` (trusted client) → vaultd `create_user_keys` → self-signed enrollment + two certificates → identityd `register_enrollment`. First enrollment per UID only.
- **Rotation/invalidation:** passphrase-authenticated identity-key signatures through vaultd. Invalidation and replacement are restrictive: the key is denied in memory and in registryd's restriction ledger before the record is made durable, and vaultd checks key status **and scope** with identityd before every signature (ADR 0006 §5). Invalidation is permitted in every state except deleted (it only narrows).
- **Key epochs and scope (revision 3, R13):** each certificate carries a `key_epoch`; registryd keeps a per-user minimum signing key epoch. `resolve_key` answers a scope (`normal`, `recovery`, `denied`): grant containment and key authority are separate facts. While a user is contained, every pre-ceremony contract key is denied; the identity key may sign only invalidations and the open ceremony's certificates; and only the ceremony's **named recovery persistent key** may sign, only trunk `restore`/`revocation` commits bound to that ceremony (`authorization.session_id = recovery_id`). The ceremony (ADR 0006 §10) generates those keys fresh in vaultd (`recover_keys`), certifies them at a new key epoch and retires every other contract key, so a key compromised in an unverifiable window cannot sign. Codes: `key_epoch_stale`, `key_retired`, `key_invalidated`, `contained`, `quarantined`, `user_deleted`, `recovery_key_required` (wire format §7).
- **Recovery and replacement (PROJECT §4.7):** M2 implements no **identity** recovery continuity. Loss of the passphrase or identity key requires `admin` `replace_identity`, which creates a **new** `user_id`; the new identity never inherits the old one's grants or keys (new genesis commits with fresh approval). A recovery mechanism that preserves identity (recovery codes or hardware authority) needs its own later decision. The ADR 0006 §10 ceremony is **grant** re-authorization after containment/quarantine by the same, still-authenticated identity (current identity key, passphrase, fresh approval); it creates new contract keys certified by that identity key and is not identity recovery.

### 3. Fresh authenticated approval before M5

Approval = the user's vault passphrase presented to vaultd for **one** registered proposal, through the trusted development client, bound to the exact bytes:

1. contractd builds the canonical payload (fresh `request_id`, expected head and policy epoch inside it) and registers it with vaultd, which independently decodes and checks it and computes `approval_digest = SHA-256(to_be_signed)`.
2. `cham3-ctl` receives the exact protected-header and payload bytes, decodes them itself with the shared verifier, displays every security-relevant field (subject, delta, lifetime, network change, parents, epoch, message) and the digest, and computes the digest itself.
3. The user confirms and types the passphrase (no echo) for that proposal; the client sends `approve_and_sign(proposal_id, approval_digest, passphrase)`.
4. vaultd requires: caller is the proposal's owner, proposal unexpired (≤ 120 s) and unused, digest equal to the registered digest, attempt limit not exceeded; then derives the KEK, decrypts, signs the Sig_structure it builds itself, zeroizes, marks the proposal used.
5. The client submits the envelope to contractd, which verifies it and asks registryd to admit it against the expected head and epoch. A head or epoch change between approval and admission rejects the commit (`stale_head`/`stale_epoch`); the user must re-approve a new proposal.

There is no unlocked-vault state: the key is decrypted only inside one signing operation. A previously used passphrase, a cached unlock or application input therefore never authorizes another transaction. This is a **knowledge-factor authentication of the human user with exact-byte binding**; it is not a trusted display or input path. A same-UID process can run the client and could observe terminal input; M3 (no access to the client) and M5 (trusted authui on a protected display path) close that gap. M2 evidence must not claim trusted human presence.

Brute force: per-UID failure counter persisted by vaultd; after 5 consecutive failures a delay of 2^(n−5) seconds (cap 1 hour) applies; success resets. Argon2id cost dominates offline guessing.

### 4. Encrypted key objects (PROJECT §4.3)

`/var/lib/cham3/vault/<uid>/<key_id hex>.key`, `cham3_dcbor_v1` map:

| Field | Value |
|---|---|
| `format_version` | 1 |
| `key_type` | `"ed25519_seed"` |
| `authority_class` | `"identity"` / `"persistent"` / `"temporary"` |
| `owner_uid`, `owner_user_id`, `key_id` | binding of the key |
| `created` | UTC time |
| `kdf` | `{"algorithm": "argon2id", "version": 19, "memory_kib", "passes", "lanes"}` |
| `algorithm` | `"xchacha20poly1305"` |
| `salt` | 16 random bytes (unique per object) |
| `nonce` | 24 random bytes |
| `ciphertext`, `tag` | 32-byte encrypted seed, 16-byte Poly1305 tag |

- AAD = deterministic encoding of the map **without** `ciphertext` and `tag`, so all metadata is authenticated. After decryption the vault recomputes the public key and requires `key_id(public) == key_id`; any mismatch is a failure (substitution detection).
- **KDF bounds:** memory 65 536–1 048 576 KiB, passes 1–10, lanes 1–4, checked **before** derivation (downgrade and denial-of-service protection). **Default** 262 144 KiB (256 MiB), 3 passes, 1 lane (RFC 9106 second recommended option scaled); calibration is fixed per release, not per host, in M2. vaultd additionally refuses any key object whose KDF memory exceeds its configured locked budget (default 512 MiB; never above `LimitMEMLOCK` minus 256 MiB of headroom).
- **KDF workspace ownership (R03):** the pinned argon2 0.5.3 clears its own intermediate hashes but **not** the caller-supplied `Block` workspace. vaultd owns that workspace through one type (the pattern demonstrated by `argon2_workspace_t` in the M2-001 probe): allocated once per derivation at exactly the parameter's block count (never grown or reallocated), resident in locked memory because the whole vault is `mlockall`ed, explicitly zeroized block by block after every derivation on success **and every error path**, zeroized again on drop, then freed. The derived KEK is written directly into a `secret_region_t`. Tests must show the workspace is all-zero after success and after each error path, and a regression control must show the library itself leaves the workspace populated. Abort or process death bypasses the zeroization; then confidentiality relies on `mlockall`, non-dumpable state and `RLIMIT_CORE=0` (§5), as for all vault secrets.
- Files `0600`, directory `0700`, owned by `_cham3-vaultd`; written by temp file, `fsync`, `rename`, directory `fsync`. Plaintext seeds never touch disk, logs or IPC.

### 5. Vault process protection (PROJECT §4.5)

At start, vaultd must successfully apply, or exit (signing unavailable, fail closed): `RLIMIT_CORE=0`, `PR_SET_DUMPABLE=0` (blocks same-UID ptrace and core dumps), `mlockall(MCL_CURRENT|MCL_FUTURE)` (no swap of transient copies; requires `LimitMEMLOCK` ≥ 1 GiB in the unit for Argon2's 256 MiB). Secrets (seed, KEK, passphrase, request buffers) live in `secret_region_t` regions. Cleanup ownership is explicit: ed25519-dalek 2.1.1's `SigningKey` and `ExpandedSecretKey` zeroize their secret fields on drop (`zeroize` feature, verified in the pinned source); argon2 0.5.3 clears only its internal hashes, so vaultd clears the Argon2 workspace itself (§4); unnamed stack temporaries inside the libraries are not erased and are covered only by `mlockall`, non-dumpable state and `RLIMIT_CORE=0` (a stated limitation; PROJECT §4.5 makes no claim that keys never enter RAM). systemd hardening: `LimitCORE=0`, `MemorySwapMax=0`, `NoNewPrivileges=yes`, empty `CapabilityBoundingSet`, `PrivateNetwork=yes`, `RestrictAddressFamilies=AF_UNIX`, `ProtectSystem=strict`, `ProtectHome=yes`, `PrivateTmp=yes`, `PrivateDevices=yes`, `MemoryDenyWriteExecute=yes`, `SystemCallFilter=@system-service` minus `@debug`. Hibernation exposure is not controllable by the vault; disabling hibernation belongs to cham3-security and is recorded as an open host-hardening obligation, not an M2 claim.

### 6. Service processes, credentials and protocol

- Five processes matching PROJECT §14 (identityd, vaultd, contractd, policyd, registryd); no consolidation proposed. Each runs as its own system user. vaultd keeps encrypted key objects; **identityd keeps no store** (R05): it validates identity objects and decides key authority, persisting them through registryd's journal and rebuilding its index from registryd at start.
- **Message sizes (R08):** every IPC message is at most 65 536 bytes and embeds at most one signed object (≤ 61 440 bytes) plus at most 4 096 bytes of other content; responses that could exceed this (history, export, identity state) are paginated by journal sequence (IPC §2).
- Transport, peer-credential roles, framing, limits and the per-operation access table are in [m2_service_ipc_v1.md](../specs/m2_service_ipc_v1.md). Roles come only from kernel-reported credentials and file-system group membership; request fields never confer roles.
- Ordinary same-UID processes cannot connect (no `_cham3-ipc` permission) and are rejected at role mapping. This is an OS DAC boundary between processes that do and do not run the trusted client; it is not application isolation (M3).

### 7. Development client passphrase custody (R03)

`cham3-ctl` collects passphrases only from its controlling terminal with echo disabled, never from arguments, environment or files. Before reading it sets `RLIMIT_CORE=0` and `PR_SET_DUMPABLE=0` (setgid execution already makes it non-dumpable) and reads directly into a `secret_region_t` (locked, guarded, not dumped). It sends the bytes once to vaultd over the `SOCK_SEQPACKET` socket (the kernel copies them into socket buffers, which are not swapped), then zeroizes the region; it never logs, echoes or stores them. Terminal echo is restored on every exit path the process controls; abort or death can leave the terminal unrestored but cannot write the passphrase anywhere. Limits: the terminal, pseudo-terminal and anything able to read them (a same-UID process, a terminal multiplexer, scrollback-capturing software) are outside this protection (§8).

### 8. Scope limits (I03)

- The setgid development client is executable by every user, and same-UID processes can run it, feed its terminal and observe its output. Its `_cham3-ipc` group and the service role checks separate processes that do and do not run that client; they do not isolate applications of the same user from each other (M3) and do not provide a trusted display, trusted input path or proof of human presence (M5).
- Passphrase knowledge with exact-digest binding authenticates a party who knows the vault passphrase for that exact proposal; it is not independently verified human presence. M2 evidence must say so.
- Application, installation, executable and session identities and the `fixture_consumer` role are M2 fixtures; they establish no runtime attribution, resource enforcement or network enforcement.

## Alternatives considered

- **PAM login password instead of a vault passphrase:** reuses existing credentials and enables PAM-integrated factors later; rejected for M2 because the vault needs the passphrase to derive the KEK anyway, and a separate PAM check would add a privileged helper without binding to the key material. PAM/FIDO2/biometric factors can wrap the KEK later (owner decision).
- **Session-long unlock with a short PIN per approval:** better usability; rejected for M2 because an unlocked key between operations widens exposure and complicates "fresh approval" semantics.
- **D-Bus with polkit:** standard desktop IPC with caller identity; rejected for the M2 core because it adds the system bus and polkit to the trusted path; a later desktop-facing broker may expose a D-Bus facade.
- **Single consolidated daemon:** smaller attack surface; rejected because vault isolation from parsing/admission code is a primary boundary. Any consolidation needs its own owner decision.
- **SO_PEERCRED UID only:** cannot distinguish the trusted client from ordinary same-UID processes; the setgid client group adds a kernel-enforced distinction at connect time.
- **Service signing key for proposals:** rejected; only user keys sign authority, and no service key is selected (ADR 0001).

## Tradeoffs and consequences

- Every change costs one passphrase entry and one Argon2id derivation (~0.3–1 s); acceptable for authorization changes.
- Passphrase entry through a terminal client is a development path; it must not be described as trusted human presence.
- Identity replacement loses continuity by design until a recovery design exists.

## Security implications

- No arbitrary signing: the vault signs only Sig_structures it builds from proposals registered by contractd and re-validated by itself, for the owner, once, after passphrase authentication (SEC-05, SEC-12).
- Cross-user reuse fails on key ownership, actor/subject binding and per-UID proposal ownership (SEC-15).
- Compromise of a privileged service (vaultd, contractd) remains outside the baseline guarantees (PROJECT §3.4).

## Verification plan (M2-004, M2-007, A04, A05, A12)

Real provider round trip and signing; tampered metadata/ciphertext/tag/nonce/salt; wrong passphrase; KDF parameters outside bounds or above the locked budget; Argon2 workspace all-zero after success and every error path (with the library regression control); client passphrase never in arguments, environment, logs or files; signing refused for invalidated, replaced, quarantined, deleted, retired and below-epoch keys after independent restores of vault files, and for contained users except exactly the ADR 0006 §10 scope (recovery positive control; stale, compromised, abandoned-attempt and deleted-user negatives; ceremony-unbound, wrong-type and admin/fixture bypass attempts; `recover_keys` crash and retry erasing unnamed key objects); locked (no passphrase) signing; canceled, expired, reused, forged, wrong-digest, wrong-owner, stale-head/epoch approvals; attempt limiting; unauthorized caller roles including same-UID ordinary processes, `admin` signing, `fixture_consumer` signing; malformed/flooded IPC; protection failures (memlock limit, non-dumpable failure) refusing service; `/proc/<pid>/smaps` lock/dontdump flags and core-limit checks; secret-pattern scans of logs, evidence and stored files. All in disposable Debian environments with synthetic users and keys.

## Approval and supersession

- **Independent review:** revision 1 CHANGES_REQUESTED at 9cf8825 (R03, R05, R08; I03 noted). Revision 2 CHANGES_REQUESTED at 336f13b (R13; R03, R05, R08 and I03 passed at design/scope level). Revision 3 requires re-review.
- **Owner approval evidence:** none. Requires independent re-review, then an explicit owner decision.
- **Supersedes / is superseded by:** revision 3 supersedes revision 2 (commit a2bbb370c867da57fdc72274fb17ca19a23426e4), which superseded revision 1 (commit f1437f0d254af2968c3598bf14d1a9b2b8e791cd).
