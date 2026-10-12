# ADR 0007: M2 lifecycle evidence, time and consumption (G5)

- **Status:** PROPOSED — awaiting independent review and owner approval. Nothing here is approved. Contains one explicit owner choice (§4).
- **Date / decision owner:** Proposed 2026-10-12 by Claude Code (M2 implementer) / cham3 project owner.
- **Related task/spec sections:** [M2 task](../tasks/M2-cryptographic-contract-core.md) G5, M2-006, A06, A09, A11, A13; PROJECT.md §§5.2, 5.3, 5.5, 5.6, 11.4; [ADR 0001](0001_eternal_contract_repository.md) approved lifecycle decision; [ADR 0002](0002_default_right_taskbar_layout.md) emergency denial; SEC-03, SEC-06, SEC-14.

`PROJECT.md` is the canonical product specification. This ADR records a scoped decision or proposal and does not silently replace that document.

## Problem and context

ADR 0001's approved lifecycle rule: expiration, session termination, one-shot consumption, key/policy invalidation, emergency restriction and fail-closed denial happen immediately — with the vault locked, the user absent, ECR unavailable or state uncertain — and never wait for a signature. Lifecycle evidence is not a user commit, cannot grant or restore authority, and its absence is reported as an audit gap. G5 must define evaluation, provenance, time and clock rollback, consumption semantics, durable ordering and reconciliation.

## Decision (proposed)

### 1. Current-authorization evaluation (policyd)

`evaluate(runtime identity, resource, operation)` is a pure function of a consistent registry snapshot, identityd key status, session liveness, mandatory policy and the trusted clock. A request is `eligible` only if **all** hold; otherwise `denied` (with reason) or `unknown` (treated as denied):

1. Mandatory policy permits the category/resource/operation and network mode for this subject (higher-level prohibitions win, PROJECT §3.3); no emergency restriction; user not quarantined.
2. A grant in the lineage's **current** state matches resource category, id, owner and operation: trunk-head grants with `persistent` lifetime, or grants with temporary lifetimes on a **live** temporary branch. A temporary branch is live only while its root is the current trunk head (any later trunk commit supersedes it) and it has not been merged, revoked or terminated.
3. Subject matches the runtime identity exactly, including package identity and executable hash; a mismatch is `unknown_identity` until an `identity_transition` commit is admitted (ADR 0004).
4. The signing key of the commit that added the grant is not invalidated as `compromised`; its certificate was valid for that commit; the commit's policy epoch ≥ the user's minimum valid epoch.
5. Lifetime: `effective_start ≤ now`; not expired (`expiration > max(now, time_floor)`); session grants only while the session source currently reports the session active (≤ 5 s old attestation); one-shot grants only with no reservation recorded.
6. Not revoked, not restricted, not removed; delegated grants additionally require the parent grant to be eligible now and to permit the delegation (scope, depth, lifetime ⊆ parent).
7. The commit is admitted; `pending`/`confirmed` status is reported separately and is never an eligibility input in M2. policyd never reports enforcement. Missing network policy evaluates Offline eligibility only.

Every rule is evaluated at request time; nothing waits for a new signature or history write.

### 2. Restrictions and explicit revocation

- **Restriction-first:** `request_revocation` causes registryd to deny the named grants in memory immediately and then append a durable `restriction` record (reserve-backed) before the user-signed `revocation` proposal is prepared. If the user cancels, the proposal expires, signing fails or admission fails, the restriction stays: access does not resume. Regaining access requires fresh authorization (a new grant id).
- **Emergency restriction** (ADR 0002 boundary): an authenticated human (trusted client) or `admin` can set a user-wide or global restriction without the vault; release is a separate authenticated operation that removes only the overlay and restores no grant that is otherwise ineligible.
- If a restrictive record cannot be made durable even with the reserve: the denial stays in memory, registryd records `degraded` in the anchor (separate rollback domain when configured), stops serving `eligible` for that user, and exits for restart; on restart a degraded flag or an unwritable store keeps the user quarantined (ADR 0006). Residual risk, stated: if both the journal and the anchor are unwritable and later recover with neither record written, the unrecorded restriction is lost after restart; the revocation was reported to the user as failed.

### 3. Lifecycle evidence and audit gaps

- Lifecycle records (`lifecycle`: expired, session_ended, consumed, consumed_uncertain, key_invalidated, policy_invalidated, superseded, branch_terminated, identity_mismatch; plus `restriction`, `use_reserved`, `use_completed`, `session_event`, `audit_gap`) are written by registryd. Provenance = the authenticated role of the reporting process (from `SO_PEERCRED`, recorded in the record) plus registryd's journal chain and anchor. **No service signing key** (ADR 0001 selects none).
- Lifecycle records only ever narrow authority. Replay rejects any lifecycle record that would grant, extend or restore (closed kind set; no "release" kind except emergency-overlay release and admin recovery records, which restore nothing by themselves).
- Expiry needs no record to take effect; registryd appends `expired` records lazily for audit when it observes them.
- When a lifecycle record cannot be written, registryd counts the gap in memory (kind, first/last time) and writes an `audit_gap` record when storage recovers. History views show gaps; nothing claims complete history.

### 4. One-shot consumption — owner choice

`reserve_use(grant, runtime, operation_id)`: under the user lock, check eligibility, append `use_reserved` (reserve-backed) and only then return a reservation token. The consumer performs the operation, then `complete_use(token, success|failure)` appends `use_completed`.

- **Option A (recommended): reservation consumes.** The grant is unusable after `use_reserved` regardless of outcome; failure, crash, timeout or unknown outcome never returns the use. Restart reports reservations without completion as `consumed_uncertain`.
- **Option B: release on confirmed failure before effect.** A consumer may report `failure_before_effect`, returning the grant. Better usability, but trusts consumer reports about effects and adds a re-activation path; rejected as default.

Concurrent consumers serialize on the user lock: exactly one obtains the reservation. A failed reservation write denies the operation.

### 5. Time and clock rollback

- Trusted time = `CLOCK_REALTIME` in registryd/policyd/contractd. registryd keeps a **time floor** = the maximum trusted time of any journal record (persisted implicitly, anchored).
- At start and on evaluation: if `now < floor − 300 s`, a clock rollback is detected: every time-bounded grant (timed, persistent-with-expiration, any grant whose `effective_start` is after `now`) is `unknown` → denied, and a `lifecycle` record notes the rollback; evaluation uses `max(now, floor)` for expiry. Persistent grants without expiration are unaffected.
- Commit timestamps must be within ±300 s of registryd's clock at admission and not earlier than any parent's timestamp minus 300 s.
- Time values never establish ordering or freshness (journal sequence does).

### 6. Sessions

Session-bound grants require positive, recent liveness from the session source; an ended session is recorded (`session_event`) and never becomes live again (session ids are random and single-use). Unknown liveness denies.

### 7. Startup reconciliation

After replay (ADR 0006): pending commits stay pending and need fresh confirmations bound to new runtime ids; reservations without completion become `consumed_uncertain`; live temporary branches whose session is not currently attested are denied until attested; clock-rollback check; degraded/quarantine handling. Nothing is re-activated, re-signed or inferred.

## Alternatives considered

- **Service-signed lifecycle evidence:** portable proof outside the store; rejected for M2 because it needs a service key with custody/rotation design (ADR 0001 requires a separate owner decision), and adds no freshness protection.
- **Timer-driven expiry records as the source of truth:** rejected; denial must not depend on a background write.
- **Temporary branches surviving unrelated trunk commits:** better usability; rejected for v1 because evaluating compatibility with intervening revisions is error-prone; any trunk commit supersedes live branches (user re-grants if needed).
- **Lifting a restriction when a revocation is canceled:** rejected; restriction is one-way.

## Tradeoffs and consequences

- Users must re-approve temporary grants after persistent changes and after recovery.
- Option A consumption can waste a use on transient failures.

## Security implications

- Denial is immediate and independent of signing, vault state, user presence and storage; durable evidence follows when possible and gaps are explicit (PROJECT §5.7).
- No lifecycle path creates authority; every authority increase still needs a user-signed, admitted commit (SEC-03, SEC-14).

## Verification plan (M2-006, A06, A09, A11, A13)

Positive controls for each grant kind; negative controls for expiry (including clock rollback), ended and unknown sessions, consumption races and kills around reservation/completion writes, compromised-key invalidation, epoch floor, restriction with canceled/failed/admission-rejected revocation, emergency overlay set/release, delegated grant after parent revocation, quarantine; all with vault locked, no user present, registry unavailable, store full (reserve) and store failing, and across restarts of release binaries.

## Approval and supersession

- **Owner approval evidence:** none yet. Requires independent review, then an explicit owner decision including §4.
- **Supersedes / is superseded by:** none. Implements ADR 0001's approved lifecycle decision.
