# ADR 0007: M2 lifecycle evidence, time and consumption (G5)

- **Status:** PROPOSED (revision 3) — awaiting independent re-review and owner approval. Nothing here is approved. Contains one explicit owner choice (§4).
- **Date / decision owner:** Proposed 2026-10-12 by Claude Code (M2 implementer); revised 2026-10-12 after the independent review published at 9cf882503848f8c6cc4938a588cc9fc062a9392d (CHANGES_REQUESTED: R04, R07, R09); revision 3 on 2026-10-12 after the re-review at 336f13be6a19bb871ea956d64f9cacb0936fe028 (CHANGES_REQUESTED: R07-r2 network uniqueness across live branches, R09-r2 trusted-time rebasing and persistence) / cham3 project owner. Revision-3 responses: [M2-001-002-finding-resolutions-r2.md](../reviews/M2-001-002-finding-resolutions-r2.md).
- **Related task/spec sections:** [M2 task](../tasks/M2-cryptographic-contract-core.md) G5, M2-006, A06, A09, A11, A13; PROJECT.md §§3.2–3.3, 5.2, 5.3, 5.5, 5.6, 11.4; [ADR 0001](0001_eternal_contract_repository.md) approved lifecycle decision; [ADR 0002](0002_default_right_taskbar_layout.md) emergency denial; SEC-03, SEC-06, SEC-14. Finding resolutions: [M2-001-002-finding-resolutions.md](../reviews/M2-001-002-finding-resolutions.md).

`PROJECT.md` is the canonical product specification. This ADR records a scoped decision or proposal and does not silently replace that document.

## Problem and context

ADR 0001's approved lifecycle rule: expiration, session termination, one-shot consumption, key/policy invalidation, emergency restriction and fail-closed denial happen immediately — with the vault locked, the user absent, ECR unavailable or state uncertain — and never wait for a signature. Lifecycle evidence is not a user commit, cannot grant or restore authority, and its absence is reported as an audit gap. Revision 2 removes revision 1's accepted residual for lost restrictions (R04), gives network authority the same lifetime/revocation model as every other grant (R07), and replaces the journal-time floor with a monotonic time model (R09). Revision 3 bounds network authority across **all live branches** of a lineage, not per state (R07-r2), and replaces revision 2's lower bounds with a rule that rebases on every accepted forward sample and persists trusted time with its BOOTTIME (R09-r2).

## Decision (proposed)

### 1. Current-authorization evaluation (policyd)

`evaluate(runtime identity, resource, operation)` is a pure function of a consistent registry snapshot (which includes identity/key state, ADR 0006 §5), session liveness, mandatory policy and the trusted time of §5. A request is `eligible` only if **all** hold; otherwise `denied` (with reason) or `unknown` (treated as denied):

1. Mandatory policy permits the category/resource/operation for this subject (higher-level prohibitions win, PROJECT §3.3); no emergency restriction; the user is neither quarantined nor contained (ADR 0006 §3–4).
2. A grant in the lineage's **current** state matches resource category, id, owner and operation: trunk-head grants with persistent lifetime, or grants with temporary lifetimes on a **live** temporary branch head. A branch is live only while its root is the current trunk head (any admitted trunk commit, including a merge, ends every live branch) and the user is not contained or quarantined; its grants end with it (wire format §6.11). No terminal lifecycle record names the grant (expired, session_ended, restriction, suspended, deleted).
3. The subject matches the runtime identity exactly, including package identity and executable hash, as bound by the lineage's authoritative subject (ADR 0004 §6.9); otherwise `unknown_identity`.
4. The signing key of the commit that added the grant was certified for that commit and is not invalidated as `compromised`; the commit's policy epoch ≥ the user's minimum valid epoch.
5. Lifetime under §5's trusted time: `effective_start ≤ now`; not expired; not latched expired; session grants only while the session source attests the session live (≤ 5 s old); one-shot grants only without a reservation.
6. Not revoked, restricted, suspended or removed; delegated grants additionally require the parent to be eligible now and to permit the delegation.
7. Admission status (`pending`/`confirmed`) is reported separately and is never an eligibility input in M2; policyd never reports enforcement.

**Network mode (R07, R07-r2).** Network authority is a grant (wire format §6.10). Admission keeps **one network slot per lineage across the trunk head and every live temporary head** (wire format §6.12): a second network grant cannot be admitted while another occupies the slot, whether on the trunk, on the same branch or on a sibling branch, whatever the lifetimes; overlapping siblings, simultaneous proposals (serialized under the admission lock: first admitted wins), timed expiry and session end (slot freed only once the terminal record is durable), restriction/revocation (freed by the durable restriction record or a signed branch revocation) and trunk supersession (ends the branches) are each frozen there with signed vectors. The lineage's effective mode is the operation (`direct` or `tor`) of its single eligible network grant; with none, or when mandatory policy forbids that mode, it is **Offline**. **Conservative evaluation:** if more than one network grant of a lineage is ever eligible, or the slot cannot be computed from a consistent snapshot (registry unavailable, contained, quarantined), the mode is **Offline** with `network_grant_conflict`/`unknown`; it is never chosen by iteration order or recency. **Transitions:** every change of the effective mode — an occupying grant ending (Direct/Tor → Offline, immediate quarantine), a new grant (Offline → Direct/Tor) or a trunk commit replacing one mode with another (Direct ↔ Tor) — is a separate enforcement transition (PROJECT §§7.7–7.8) that starts from deny-egress quarantine and is never reported as enforced by M2. Network grants therefore expire, end with their session, are restricted, revoked, invalidated with their signing key or policy epoch, suspended by containment and matched by restore like any other grant. Evaluation reports eligibility only; network enforcement and transition safety are M4/M6.

Every rule is evaluated at request time; nothing waits for a new signature or history write.

### 2. Restrictions and explicit revocation

- **Restriction-first:** `request_revocation` (and every other restrictive change) is denied in memory immediately, recorded in registryd's restriction ledger, then made durable (ADR 0006 §3) before the user-signed `revocation` proposal is prepared. For a persistent grant the signed commit is a trunk `revocation`; for a temporary grant it is a **branch revocation** whose parent is that branch's live head and which removes only grants added on that branch (wire format §6.11). The durable restriction record alone already ends the grant id permanently (and frees a network slot). If the user cancels, the proposal expires, signing fails, admission fails or storage is unwritable, the restriction stays. Regaining access requires fresh authorization with a new grant id.
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

### 5. Trusted time (R09, R09-r2)

Revision 3 replaces revision 2's lower bounds, which stopped advancing after a forward sample followed by a backward correction and persisted raw realtime floors (R09-r2). One rule, stated in terms of the **offset** `trusted − CLOCK_BOOTTIME`:

1. **Maximum offset per boot.** For the current `boot_id`, `offset` is the largest `realtime − boottime` ever accepted in this boot by any cham3 service, from any persisted floor or ledger entry of this boot. **Trusted now = boottime + offset.** CLOCK_BOOTTIME keeps counting through suspend and never steps, so trusted time advances at least as fast as real elapsed time since the last accepted sample, through idle periods and suspend.
2. **Rebase on every accepted forward sample.** Each evaluation samples `(realtime, boottime)` together. If `realtime > boottime + offset`, the sample is accepted and `offset := realtime − boottime` — trusted time continues from that observation (the re-review's 14:00 example reaches 15:00 an hour later, whatever the wall clock is corrected to). A sample behind trusted time changes nothing; more than 300 s behind is a `clock_anomaly` record. Trusted time never moves backward within a boot.
3. **Persist trusted time, not realtime.** `time_floor` records and restriction-ledger time entries store `{boot_id, boottime, trusted}` (equivalently the offset). Every offset increase is appended to the tmpfs restriction ledger immediately; registryd writes a `time_floor` journal record at startup, at most every 60 s while the offset grows, with every other record and before an orderly shutdown. A writer never persists a value below the one it holds; a reader takes the **maximum** offset over all floors and ledger entries of the current boot, so a lower or out-of-order floor can never lower trusted time.
4. **One offset shared between services.** registryd owns the persisted offset. policyd evaluates with `max(own offset, offset of the registry snapshot it evaluates against)`; before relying on an offset increase it reports it to registryd (`time_sample`, IPC spec), which takes the maximum and ledgers it before replying. A restarting policyd or registryd in the same boot starts from that maximum, so services cannot disagree downward and neither restart nor clean shutdown loses an observation.
5. **Expiry latch.** When evaluation observes a grant expired, the expiry is latched: denied in memory, ledger entry, then a durable `expired` record (restrictive path). A later correction cannot un-expire it.
6. **New boot** (limits retained). Elapsed time across a reboot cannot be measured without an external time source. On a new `boot_id`: every live temporary grant (timed, session, one-shot) ends (`branch_terminated`); the initial offset is `max(realtime − boottime, last persisted trusted value of any earlier boot − boottime)`, so trusted time starts no earlier than the last trusted value; persistent grants with an expiration are evaluated against trusted now. Stated limit: an administrator who sets the hardware clock back across a reboot, before any newer time is observed, can extend a persistent expiration by at most the interval since the last persisted floor; a forward wall-clock step makes grants expire earlier (fail-closed) and can make a future `effective_start` begin early, which is bounded by the commit-time rule below; the kernel and root control the clock (PROJECT §3.4). An authenticated time source (for example NTS) is a later decision.

Executable reference: `tests/m2/oracle/trusted_time_model.py`, with `tests/m2/harness/test_m2_trusted_time.py` covering the re-review's forward step → backward correction → long idle and suspend, same-boot restart from journal floor and from a newer ledger entry, clean shutdown, regressing floors, two services, the new-boot start and a seeded monotonicity property; the revision-2 bounds are kept there as a failing contrast. Model evidence only; the service clock is NOT_RUN.

Commit timestamps must be within ±300 s of trusted now at admission and not earlier than any parent's timestamp minus 300 s. Time values never establish ordering or freshness (journal sequence does).

### 6. Sessions

Session-bound grants (including temporary network grants) require positive, recent liveness from the session source; ended sessions are recorded and never become live again (session ids are random and single-use). Unknown liveness denies. All sessions end at a new boot.

### 7. Startup reconciliation

After the run-marker, ledger and anchor decisions (ADR 0006 §3–4) and replay: pending commits stay pending and need fresh confirmations bound to new runtime ids; reservations without completion become `consumed_uncertain`; new-boot rules of §5 (trusted time starts from the maximum persisted offset); contained/quarantined users remain denied until the recovery ceremony (ADR 0006 §10) re-authorizes scope with new grant ids; an open ceremony resumes at its first incomplete step. Nothing is re-activated, re-signed or inferred.

## Alternatives considered

- **Persistent-only network policy as a top-level state field (revision 1):** simpler, but lacks grant ids, lifetimes and grant-specific revocation; rejected per R07. An owner-approved scope reduction would have been required to keep it.
- **Journal-time floor only (revision 1):** misses rollback after idle periods and failed writes; rejected per R09.
- **Lower bounds from process start plus every observed realtime, raw-realtime floors (revision 2):** freezes after a forward sample and backward correction and can regress on restart; rejected per R09-r2.
- **Per-state network uniqueness only (revision 2):** admits Direct and Tor on live sibling branches; rejected per R07-r2. **Picking the most recent or first-listed eligible network grant at evaluation:** makes the approved mode depend on iteration order; rejected for admission-time exclusion plus Offline on any conflict.
- **Releasing the network slot when a lifetime has passed by clock arithmetic at admission:** admission must replay deterministically from durable records; rejected for durable terminal records.
- **Service-signed lifecycle evidence; timer-driven expiry records as source of truth; temporary branches surviving trunk commits; lifting a restriction when a revocation is canceled:** rejected as in revision 1.
- **Keeping temporary grants across reboot:** rejected because elapsed time across a reboot is unverifiable without an external source.

## Tradeoffs and consequences

- Users re-approve temporary grants after persistent changes, after reboots and after containment.
- Option A consumption can waste a use on transient failures.
- Temporary network grants exist only for lineages without a persistent network grant, and at most one at a time per lineage; a user who wants to switch a temporary branch from Direct to Tor ends the first grant (expiry, session end, restriction or branch revocation) before the second is admitted.

## Security implications

- Denial is immediate, independent of signing, vault state, user presence and storage, and now survives write failures and restarts (PROJECT §5.7).
- Network authority gains the same bounded, revocable lifecycle as every other permission.
- Trusted time never moves backwards within a boot, continues from every accepted forward observation at BOOTTIME pace, survives restarts and is shared between services; observed expiries are latched; cross-boot limits are stated.
- At most one network grant is ever eligible per lineage; any inconsistency evaluates Offline.
- No lifecycle path creates authority (SEC-03, SEC-14).

## Verification plan (M2-006, A06, A09, A11, A13)

All planned; nothing has run except the oracle network-slot scenarios (wire format §6.12) and the trusted-time reference model (§5), which are model evidence. Network slot under concurrent admissions of sibling proposals, with expiry/session/restriction records failing to persist; positive controls for each grant kind including network grants; expiry under wall-clock rollback during a run, after a long record-free interval, after a same-boot service restart, across suspend/resume (BOOTTIME) and after failed expiry evidence; new-boot termination of temporary grants; ended/unknown sessions; consumption races and kills around reservation/completion; compromised-key invalidation; epoch floor; restriction with canceled/failed/rejected revocation and with journal and anchor both unwritable followed by same-boot and new-boot restarts; emergency overlay set/release; delegated grant after parent revocation; containment and quarantine — each with the vault locked, no user present, registry unavailable or full, across restarts of release binaries.

## Approval and supersession

- **Independent review:** revision 1 CHANGES_REQUESTED at 9cf8825 (R04, R07, R09). Revision 2 CHANGES_REQUESTED at 336f13b (R07-r2, R09-r2; R04 passed at design level). Revision 3 requires re-review.
- **Owner approval evidence:** none. Requires independent re-review, then an explicit owner decision including §4.
- **Supersedes / is superseded by:** revision 3 supersedes revision 2 (commit a2bbb370c867da57fdc72274fb17ca19a23426e4), which superseded revision 1 (commit f1437f0d254af2968c3598bf14d1a9b2b8e791cd). Implements ADR 0001's approved lifecycle decision.
