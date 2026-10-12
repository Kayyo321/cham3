# ADR 0005: M2 identity, authentication, vault and service IPC (G3)

- **Status:** PROPOSED — awaiting independent review and owner approval. Nothing here is approved.
- **Date / decision owner:** Proposed 2026-10-12 by Claude Code (M2 implementer) / cham3 project owner.
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
| Signing keys | identityd registry of enrollments, certificates and invalidations signed by the user's identity key. Caller-supplied key ids are only lookups. | |
| Application / installation / executable | **Fixture in M2**: supplied by the trusted client and bound into the signed subject. Real attribution (launcher, package database, executable hashing) is M3. | Labeled fixture; establishes no isolation or attribution claim. |
| Desktop/app session | Session events and liveness from the `fixture_consumer` role standing in for the future cham3-sessiond; unknown liveness denies. | Labeled fixture. |
| Initiating program | Derived by contractd from the caller role (`cham3_dev_cli` for `trusted_client` in M2), never from request text. | |

### 2. Key hierarchy and lifecycle

- Per user: one **identity** key (certifies keys, signs invalidations, never signs commits), one **persistent** contract key and one **temporary** contract key (certificate `not_after` ≤ 7 days, re-certified on rotation). All Ed25519, generated in vaultd from `getrandom(2)` into locked memory.
- Authority rules (identityd layer of verification): `temporary_grant` commits may be signed by temporary or persistent keys; every other commit type requires a persistent key; certificates/invalidations require the user's current identity key; a key's owner must equal the commit's `actor`/`subject`; temporary keys are rejected outside their certificate window; an invalidated key with reason `compromised` makes every commit it signed ineligible (ADR 0007); `rotated`/`retired` keys stop signing but their earlier commits stay verifiable and eligible.
- **Enrollment:** `cham3-ctl enroll` (trusted client) → vaultd `create_user_keys` → self-signed enrollment + two certificates → identityd `register_enrollment`. First enrollment per UID only.
- **Rotation/invalidation:** passphrase-authenticated identity-key signatures through vaultd.
- **Recovery and replacement (PROJECT §4.7):** M2 implements no recovery continuity. Loss of the passphrase or identity key requires `admin` `replace_identity`, which creates a **new** `user_id`; the new identity never inherits the old one's grants or keys (new genesis commits with fresh approval). A recovery mechanism that preserves identity (recovery codes or hardware authority) needs its own later decision.

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
- **KDF bounds:** memory 65 536–1 048 576 KiB, passes 1–10, lanes 1–4, checked **before** derivation (downgrade and denial-of-service protection). **Default** 262 144 KiB (256 MiB), 3 passes, 1 lane (RFC 9106 second recommended option scaled); calibration is fixed per release, not per host, in M2.
- Files `0600`, directory `0700`, owned by `_cham3-vaultd`; written by temp file, `fsync`, `rename`, directory `fsync`. Plaintext seeds never touch disk, logs or IPC.

### 5. Vault process protection (PROJECT §4.5)

At start, vaultd must successfully apply, or exit (signing unavailable, fail closed): `RLIMIT_CORE=0`, `PR_SET_DUMPABLE=0` (blocks same-UID ptrace and core dumps), `mlockall(MCL_CURRENT|MCL_FUTURE)` (no swap of transient copies; requires `LimitMEMLOCK` ≥ 1 GiB in the unit for Argon2's 256 MiB). Secrets (seed, KEK, passphrase, request buffers) live in `secret_region_t` regions; ed25519-dalek/argon2 transient copies are covered by `mlockall` and their own zeroization. systemd hardening: `LimitCORE=0`, `MemorySwapMax=0`, `NoNewPrivileges=yes`, empty `CapabilityBoundingSet`, `PrivateNetwork=yes`, `RestrictAddressFamilies=AF_UNIX`, `ProtectSystem=strict`, `ProtectHome=yes`, `PrivateTmp=yes`, `PrivateDevices=yes`, `MemoryDenyWriteExecute=yes`, `SystemCallFilter=@system-service` minus `@debug`. Hibernation exposure is not controllable by the vault; disabling hibernation belongs to cham3-security and is recorded as an open host-hardening obligation, not an M2 claim.

### 6. Service processes, credentials and protocol

- Five processes matching PROJECT §14 (identityd, vaultd, contractd, policyd, registryd); no consolidation proposed. Each runs as its own system user with its own state directory.
- Transport, peer-credential roles, framing, limits and the per-operation access table are in [m2_service_ipc_v1.md](../specs/m2_service_ipc_v1.md). Roles come only from kernel-reported credentials and file-system group membership; request fields never confer roles.
- Ordinary same-UID processes cannot connect (no `_cham3-ipc` permission) and are rejected at role mapping. This is an OS DAC boundary between processes that do and do not run the trusted client; it is not application isolation (M3).

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

Real provider round trip and signing; tampered metadata/ciphertext/tag/nonce/salt; wrong passphrase; KDF parameters outside bounds; locked (no passphrase) signing; canceled, expired, reused, forged, wrong-digest, wrong-owner, stale-head/epoch approvals; attempt limiting; unauthorized caller roles including same-UID ordinary processes, `admin` signing, `fixture_consumer` signing; malformed/flooded IPC; protection failures (memlock limit, non-dumpable failure) refusing service; `/proc/<pid>/smaps` lock/dontdump flags and core-limit checks; secret-pattern scans of logs, evidence and stored files. All in disposable Debian environments with synthetic users and keys.

## Approval and supersession

- **Owner approval evidence:** none yet. Requires independent review, then an explicit owner decision.
- **Supersedes / is superseded by:** none.
