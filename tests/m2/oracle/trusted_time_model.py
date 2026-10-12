"""Reference model of the trusted-time rule of ADR 0007 section 5 (revision 3, R09-r2).

This is an executable statement of the proposed design used by the harness regression
tests; it is not the policyd/registryd clock and it reads no system clock. All values
are integer seconds: `realtime` is a CLOCK_REALTIME sample, `boottime` the
CLOCK_BOOTTIME sample taken with it (counts through suspend, restarts at each boot).

Rule (maximum offset): per boot, trusted time is `boottime + offset` where `offset` is
the largest `realtime - boottime` ever accepted in this boot, from any service and from
any persisted floor or ledger entry of this boot. Every sample whose realtime is ahead
of trusted time is accepted and rebases trusted time to it; a sample behind trusted time
changes nothing (and is an anomaly when more than AnomalySeconds behind). Floors persist
the trusted value with its BOOTTIME (never raw realtime), so a restart in the same boot
resumes from the maximum and keeps counting with BOOTTIME. A new boot cannot measure
elapsed time: trusted time starts no earlier than the last persisted trusted value.

Status: PROPOSED G5 design (ADR 0007, not approved). Status of the real clock: NOT_RUN.
"""

AnomalySeconds = 300


class floor_t:
    """A persisted `time_floor` journal record or ledger entry."""

    def __init__(self, boot_id, boottime, trusted):
        self.boot_id = boot_id
        self.boottime = boottime
        self.trusted = trusted

    def offset(self):
        return self.trusted - self.boottime


class trusted_clock_t:
    """One service's view of trusted time in one boot."""

    def __init__(self, boot_id, realtime, boottime, floors=()):
        self.boot_id = boot_id
        same_boot = [floor for floor in floors if floor.boot_id == boot_id]
        earlier_boots = [floor for floor in floors if floor.boot_id != boot_id]
        self.offset = realtime - boottime
        # Same boot: resume from the largest persisted offset (a lower floor can never
        # lower trusted time, whatever its position in the journal).
        for floor in same_boot:
            self.offset = max(self.offset, floor.offset())
        # New boot: elapsed time across the reboot is unknown; never start earlier than
        # the last trusted value persisted by any earlier boot.
        if earlier_boots and not same_boot:
            last_trusted = max(floor.trusted for floor in earlier_boots)
            self.offset = max(self.offset, last_trusted - boottime)
        self.anomalies = []
        self.last_boottime = boottime

    def now(self, boottime):
        if boottime < self.last_boottime:
            raise ValueError("CLOCK_BOOTTIME never decreases within a boot")
        self.last_boottime = boottime
        return boottime + self.offset

    def sample(self, realtime, boottime):
        """Accepts one (realtime, boottime) observation; returns trusted now."""
        trusted = self.now(boottime)
        if realtime > trusted:
            self.offset = realtime - boottime  # accepted forward sample: rebase
            return realtime
        if trusted - realtime > AnomalySeconds:
            self.anomalies.append((boottime, trusted - realtime))
        return trusted

    def adopt(self, offset):
        """Takes a peer's (or the registry snapshot's) offset; only ever increases."""
        self.offset = max(self.offset, offset)

    def floor(self, boottime):
        """The value persisted by a time_floor record, ledger entry or clean shutdown."""
        return floor_t(self.boot_id, boottime, self.now(boottime))

    def expired(self, expiration, boottime):
        return self.now(boottime) >= expiration


def revision_2_high_water(start_realtime, start_boottime, observations):
    """The revision-2 lower bounds the re-review disproved (for regression contrast):
    max(realtime0 + elapsed BOOTTIME, every realtime observed)."""
    high = start_realtime
    for realtime, boottime in observations:
        high = max(high, start_realtime + (boottime - start_boottime), realtime)
    return high
