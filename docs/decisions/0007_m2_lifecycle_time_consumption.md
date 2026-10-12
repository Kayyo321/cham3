# ADR 0007: M2 lifecycle evidence, time and consumption (G5)

- **Status:** PROPOSED (revision 2) — awaiting independent re-review and owner approval. Nothing here is approved. Contains one explicit owner choice (§4).
- **Date / decision owner:** Proposed 2026-10-12 by Claude Code (M2 implementer); revised 2026-10-12 after the independent review published at 9cf882503848f8c6cc4938a588cc9fc062a9392d (CHANGES_REQUESTED: R04, R07, R09) / cham3 project owner.
- **Related task/spec sections:** [M2 task](../tasks/M2-cryptographic-contract-core.md) G5, M2-006, A06, A09, A11, A13; PROJECT.md §§3.2–3.3, 5.2, 5.3, 5.5, 5.6, 11.4; [ADR 0001](0001_eternal_contract_repository.md) approved lifecycle decision; [ADR 0002](0002_default_right_taskbar_layout.md) emergency denial; SEC-03, SEC-06, SEC-14. Finding resolutions: [M2-001-002-finding-resolutions.md](../reviews/M2-001-002-finding-resolutions.md).

`PROJECT.md` is the canonical product specification. This ADR records a scoped decision or proposal and does not silently replace that document.

## Problem and context

ADR 0001's approved lifecycle rule: expiration, session termination, one-shot consumption, key/policy invalidation, emergency restriction and fail-closed denial happen immediately — with the vault locked, the user absent, ECR unavailable or state uncertain — and never wait for a signature. Lifecycle evidence is not a user commit, cannot grant or restore authority, and its absence is reported as an audit gap. Revision 2 removes revision 1's accepted residual for lost restrictions (R04), gives network authority the same lifetime/revocation model as every other grant (R07), and replaces the journal-time floor with a monotonic time model that cannot be undone by later clock correction (R09).

## Decision (proposed)

### 1. Current-authorization evaluation (policyd)

`evaluate(runtime identity, resource, operation)` is a pure function of a consistent registry snapshot (which includes identity/key state, ADR 0006 §5), session liveness, mandatory policy and the trusted time of §5. A request is `eligible` only if **all** hold; otherwise `denied` (with reason) or `unknown` (treated as denied):

1. Mandatory policy permits the category/resource/operation for this subject (higher-level prohibitions win, PROJECT §3.3); no emergency restriction; the user is neither quarantined nor contained (ADR 0006 §3–4).
2. A grant in the lineage's **current** state matches resource category, id, owner and operation: trunk-head grants with persistent lifetime, or grants with temporary lifetimes on a **live** temporary branch. A branch is live only while its root is the current trunk head and it has not been merged, revoked or terminated.
3. The subject matches the runtime identity exactly, including package identity and executable hash, as bound by the lineage's authoritative subject (ADR 0004 §6.9); otherwise `unknown_identity`.
4. The signing key of the commit that added the grant was certified for that commit and is not invalidated as `compromised`; the commit's policy epoch ≥ the user's minimum valid epoch.
5. Lifetime under §5's trusted time: `effective_start ≤ now`; not expired; not latched expired; session grants only while the session source attests the session live (≤ 5 s old); one-shot grants only without a reservation.
6. Not revoked, restricted, suspended or removed; delegated grants additionally require the parent to be eligible now and to permit the delegation.
7. Admission status (`pending`/`confirmed`) is reported separately and is never an eligibility input in M2; policyd never reports enforcement.

**Network mode (R07).** Network authority is a grant (ADR 0004 §6.10). The lineage's effective mode is the operation (`direct` or `tor`) of its single eligible network grant; with none, or when mandatory policy forbids that mode, it is **Offline**. Network grants therefore expire, end with their session, are restricted, revoked, invalidated with their signing key or policy epoch, suspended by containment and matched by restore like any other grant. A temporary Direct/Tor grant is possible only for a lineage whose trunk state has no network grant. Evaluation reports eligibility only; network enforcement and transition safety are M4/M6.

Every rule is evaluated at request time; nothing waits for a new signature or history write.

### 2. Restrictions and explicit revocation

- **Restriction-first:** `request_revocation` (and every other restrictive change) is denied in memory immediately, recorded in registryd's restriction ledger, then made durable (ADR 0006 §3) before the user-signed `revocation` proposal is prepared. If the user cancels, the proposal expires, signing fails, admission fails or storage is unwritable, the restriction stays. Regaining access requires fresh authorization with a new grant id.
- **Persistence across restarts (R04):** a restriction that could not be made durable is re-applied from the ledger after a same-boot restart, and any unverifiable unclean run (including a reboot) contains every grant until fresh authorization (ADR 0006 §3). Revision 1's accepted residual is withdrawn.
- **Emergency restriction** (ADR 0002 boundary): an authenticated human (trusted client) or `admin` can set a user-wide or global restriction without the vault; it follows the same restrictive path. Release is a separate authenticated operation that removes only the overlay and restores no grant that is otherwise ineligible.

### 3. Lifecycle evidence and audit gaps

- Lifecycle records (expired, session_ended, consumed, consumed_uncertain, key_invalidated, policy_invalidated, superseded, branch_terminated, identity_mismatch, clock_anomaly, suspended; plus restriction, use_reserved, use_completed, session_event, time_floor, audit_gap) are written by registryd with the authenticated role of the reporting process (`SO_PEERCRED`) and integrity from the journal chain and anchor. **No service signing key** (ADR 0001 selects none).
- Lifecycle records only narrow authority; replay rejects any record kind that would grant, extend or restore. Emergency-overlay release and admin recovery records restore nothing by themselves.
- When a record cannot be written, the gap is counted in memory and in the ledger, and an `audit_gap` record is written when storage recovers. Nothing claims complete history.

### 4. One-shot consumption — owner choice

`reserve_use(grant, runtime, operation_id)`: under the user lock check eligibility, deny further use in memory, append to the ledger, append `use_reserved` durably, then return the reservation. The consumer performs the operation and reports `complete_use(token, success|failure)`.

- **Option A (recommended): reservation consumes.** After `use_reserved` the grant is unusable regardless of outcome; failure, crash, timeout or unknown outcome never returns the use. Restart reports reservations without completion as `consumed_uncertain`.
- **Option B: release on confirmed failure-before-effect.** A consumer may report `failure_before_effect`, returning the grant; trusts consumer reports and adds a re-activation path; not recommended.

Concurrent consumers serialize on the user lock; exactly one obtains the reservation. A failed reservation write still consumes in memory and in the ledger, and denies the operation.

### 5. Trusted time (R09)

policyd and registryd use one rule: **trusted now = max(realtime, high-water)**, where the high-water mark only moves forward:

1. **In-process monotonic high-water.** At start each process records `(realtime₀, boottime₀)` with `CLOCK_BOOTTIME` (which keeps counting through suspend). Thereafter `high_water ≥ realtime₀ + (boottime − boottime₀)` and `high_water ≥` every realtime value previously observed. Setting the wall clock back during a run therefore never moves trusted time back, including across suspend/resume.
2. **Persisted floor.** registryd appends a `time_floor` record `{realtime, boot_id, boottime}` at startup, at most every 60 s while time advances, before orderly shutdown, and with every other record. After a **same-boot** restart, the floor bounds time from below by `floor.realtime + (boottime − floor.boottime)`, so time that passed while a service was down is not lost.
3. **Expiry latch.** When evaluation observes a grant expired, the expiry is latched: denied in memory, ledger entry, then a durable `expired` record (restrictive path). A later clock correction cannot un-expire it; failed persistence is covered by the ledger/containment rules (ADR 0006 §3).
4. **Detection.** If realtime is more than 300 s behind the in-process high-water or the same-boot floor bound, a `clock_anomaly` is recorded and evaluation continues with trusted now (fail-closed for expiries).
5. **New boot.** Elapsed time across a reboot cannot be measured without an external time source. On a new `boot_id`: every live temporary grant (timed, session, one-shot) ends (`branch_terminated`); trusted now ≥ the last persisted floor realtime; persistent grants with an expiration are evaluated against trusted now. Stated limit: an administrator who sets the hardware clock back across a reboot, before any newer time has been observed, can extend a persistent expiration by at most the interval since the last persisted floor; the kernel and root control the clock (PROJECT §3.4). An authenticated time source (for example NTS) is a later decision.

Commit timestamps must be within ±300 s of trusted now at admission and not earlier than any parent's timestamp minus 300 s. Time values never establish ordering or freshness (journal sequence does).

### 6. Sessions

Session-bound grants (including temporary network grants) require positive, recent liveness from the session source; ended sessions are recorded and never become live again (session ids are random and single-use). Unknown liveness denies. All sessions end at a new boot.

### 7. Startup reconciliation

After the run-marker, ledger and anchor decisions (ADR 0006 §3–4) and replay: pending commits stay pending and need fresh confirmations bound to new runtime ids; reservations without completion become `consumed_uncertain`; new-boot rules of §5; contained/quarantined users remain denied. Nothing is re-activated, re-signed or inferred.

## Alternatives considered

- **Persistent-only network policy as a top-level state field (revision 1):** simpler, but lacks grant ids, lifetimes and grant-specific revocation; rejected per R07. An owner-approved scope reduction would have been required to keep it.
- **Journal-time floor only (revision 1):** misses rollback after idle periods and failed writes; rejected per R09.
- **Service-signed lifecycle evidence; timer-driven expiry records as source of truth; temporary branches surviving trunk commits; lifting a restriction when a revocation is canceled:** rejected as in revision 1.
- **Keeping temporary grants across reboot:** rejected because elapsed time across a reboot is unverifiable without an external source.

## Tradeoffs and consequences

- Users re-approve temporary grants after persistent changes, after reboots and after containment.
- Option A consumption can waste a use on transient failures.
- Temporary network grants exist only for lineages without a persistent network grant.

## Security implications

- Denial is immediate, independent of signing, vault state, user presence and storage, and now survives write failures and restarts (PROJECT §5.7).
- Network authority gains the same bounded, revocable lifecycle as every other permission.
- Trusted time never moves backwards within a boot; observed expiries are latched; cross-boot limits are stated.
- No lifecycle path creates authority (SEC-03, SEC-14).

## Verification plan (M2-006, A06, A09, A11, A13)

All planned; nothing has run. Positive controls for each grant kind including network grants; expiry under wall-clock rollback during a run, after a long record-free interval, after a same-boot service restart, across suspend/resume (BOOTTIME) and after failed expiry evidence; new-boot termination of temporary grants; ended/unknown sessions; consumption races and kills around reservation/completion; compromised-key invalidation; epoch floor; restriction with canceled/failed/rejected revocation and with journal and anchor both unwritable followed by same-boot and new-boot restarts; emergency overlay set/release; delegated grant after parent revocation; containment and quarantine — each with the vault locked, no user present, registry unavailable or full, across restarts of release binaries.

## Approval and supersession

- **Independent review:** revision 1 CHANGES_REQUESTED at 9cf8825 (R04, R07, R09). Revision 2 requires re-review.
- **Owner approval evidence:** none. Requires independent re-review, then an explicit owner decision including §4.
- **Supersedes / is superseded by:** revision 2 supersedes revision 1 of this proposal (commit f1437f0d254af2968c3598bf14d1a9b2b8e791cd). Implements ADR 0001's approved lifecycle decision.
