//! Trusted time (ADR 0007 §5; §8 option B; erratum E6).
//!
//! **Rule (maximum offset per boot).** Each observation samples `(realtime,
//! boottime)` together: CLOCK_REALTIME and CLOCK_BOOTTIME, in whole seconds.
//! CLOCK_BOOTTIME counts through suspend and never steps. Trusted now is
//! `boottime + offset`, where `offset` is the largest `realtime − boottime` accepted
//! in this boot by any cham3 service, from any persisted floor or ledger entry of
//! this boot. A sample ahead of trusted time is accepted and rebases the offset to
//! it. A sample behind trusted time changes nothing; more than 300 s behind, it is a
//! clock anomaly. Trusted time never moves backward within a boot.
//!
//! **Persistence (rule 3, erratum E6).** A floor `{boot_id, boottime, trusted,
//! observed}` is a field of the global anchor image ([`crate::anchor`]), and every
//! offset increase is a time entry of the restriction ledger ([`crate::ledger`]). A
//! restart in the same boot resumes from the maximum offset over the floors of this
//! boot. A new boot cannot measure the time that passed across the reboot, so
//! trusted time starts no earlier than the last trusted value an earlier boot
//! persisted.
//!
//! **Observed-time counter (§8 option B).** This counts the CLOCK_BOOTTIME seconds
//! cham3 has observed. It is persisted with every floor, resumed in the same boot
//! from the maximum and at a new boot from the last persisted value. Wall-clock
//! samples and powered-off time never advance it. An admitted commit records it
//! (`commit_admitted.observed`), and a future `effective_start` is eligible only once
//! enough observed time has passed since admission (`cham3_policyd`).
//!
//! **Anomaly episodes (erratum E6; M2-006-B1-R02).** The episode state is persisted
//! with every floor as one monotonic mark (`2·episode` while open, `2·episode + 1`
//! once closed), so the larger mark is always the later state. A same-boot restart
//! resumes the episode, open or closed, and a `clock_anomaly` record names its
//! episode, so replay knows which users have reported it.
//!
//! The executable reference is `tests/m2/oracle/trusted_time_model.py`. The
//! `trusted_time_model` cargo test checks this clock against vectors that model
//! produced.
//!
//! **Time source.** [`kernel_time_source_t`] reads CLOCK_BOOTTIME from
//! `/proc/uptime`, which the kernel derives from its boot-time clock, suspend
//! included. This is an author placement, open to review: it changes no
//! dependency. The alternative, rustix's `time` feature, would change ADR 0003's
//! dependency record. The realtime sample is taken between two uptime reads and
//! paired with the later one, so sampling never makes the offset appear larger than
//! it is.

use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

/// A sample more than this many seconds behind trusted time is a clock anomaly (§5
/// rule 2).
pub const AnomalySeconds: u64 = 300;
/// A commit timestamp must be within this many seconds of trusted now (§5; erratum
/// E4, `timestamp_rule`).
pub const AdmissionSkewSeconds: u64 = 300;
/// registryd persists a floor at least this often while operations observe the
/// clock (§5 rule 3).
pub const FloorIntervalSeconds: u64 = 60;
/// Attempts at a consistent `(realtime, boottime)` pair.
const SampleAttempts: usize = 3;
/// Largest CLOCK_BOOTTIME step between the two reads of one sample.
const MaxSampleSpreadSeconds: u64 = 1;

/// One observation of the two clocks, in whole seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct time_sample_t {
    /// CLOCK_REALTIME: seconds since 1970-01-01T00:00:00Z.
    pub realtime: u64,
    /// CLOCK_BOOTTIME: seconds since this boot, suspend included.
    pub boottime: u64,
}

/// Where samples come from.
pub trait time_source_t: Send + Sync {
    /// One sample, or `None` when it cannot be taken (the caller fails closed).
    fn sample(&self) -> Option<time_sample_t>;
}

/// The kernel's clocks: CLOCK_REALTIME from the standard library, CLOCK_BOOTTIME
/// from `/proc/uptime`.
#[derive(Clone, Copy, Debug, Default)]
pub struct kernel_time_source_t;

fn read_boottime() -> Option<u64> {
    let text = fs::read_to_string("/proc/uptime").ok()?;
    let first = text.split_whitespace().next()?;
    let whole = first.split('.').next()?;
    whole.parse().ok()
}

fn read_realtime() -> Option<u64> {
    Some(SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs())
}

impl time_source_t for kernel_time_source_t {
    fn sample(&self) -> Option<time_sample_t> {
        for _ in 0..SampleAttempts {
            let before = read_boottime()?;
            let realtime = read_realtime()?;
            let after = read_boottime()?;
            if after >= before && after.checked_sub(before)? <= MaxSampleSpreadSeconds {
                return Some(time_sample_t {
                    realtime,
                    boottime: after,
                });
            }
        }
        None
    }
}

/// A persisted floor (the anchor's, or a ledger time entry): trusted and observed
/// time at a CLOCK_BOOTTIME instant of one boot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct time_floor_t {
    pub boot_id: String,
    pub boottime: u64,
    pub trusted: u64,
    pub observed: u64,
    /// The anomaly episode state of this boot ([`anomaly_state_t::mark`]).
    pub anomaly: u64,
}

fn signed(value: u64) -> Option<i64> {
    i64::try_from(value).ok()
}

fn at(boottime: u64, offset: i64) -> Option<u64> {
    u64::try_from(signed(boottime)?.checked_add(offset)?).ok()
}

impl time_floor_t {
    fn offset(&self) -> Option<i64> {
        signed(self.trusted)?.checked_sub(signed(self.boottime)?)
    }

    fn observed_offset(&self) -> Option<i64> {
        signed(self.observed)?.checked_sub(signed(self.boottime)?)
    }
}

/// The later of two floors. For one boot, the later CLOCK_BOOTTIME with the larger
/// offset and observed offset, so that no write can persist less than an earlier
/// write did, whatever order concurrent writers reach the anchor in. A floor of
/// another boot is replaced by the newer boot's, which starts no earlier than it.
pub fn merge_floor(existing: Option<time_floor_t>, next: time_floor_t) -> time_floor_t {
    let Some(existing) = existing else {
        return next;
    };
    if existing.boot_id != next.boot_id {
        return next;
    }
    let merged = (|| {
        let boottime = existing.boottime.max(next.boottime);
        let offset = existing.offset()?.max(next.offset()?);
        let observed_offset = existing.observed_offset()?.max(next.observed_offset()?);
        Some(time_floor_t {
            boot_id: next.boot_id.clone(),
            boottime,
            trusted: at(boottime, offset)?,
            observed: at(boottime, observed_offset)?,
            anomaly: existing.anomaly.max(next.anomaly),
        })
    })();
    merged.unwrap_or(next)
}

/// What one sample means for the clock, before anything changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct assessment_t {
    /// The sample is ahead of trusted time: the offset it would rebase to. It is
    /// adopted only once it is in the ledger (rule 3).
    pub raise: Option<i64>,
    /// The sample is more than [`AnomalySeconds`] behind trusted time, by this much.
    pub behind: Option<u64>,
}

/// The clock's shared state for `lineage_head`: policyd evaluates with the larger of
/// its own offset and this one (rule 4), and with the observed counter's offset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct time_view_t {
    pub boot_id: String,
    pub offset: i64,
    pub observed_offset: i64,
}

impl time_view_t {
    /// Trusted now at `boottime`.
    pub fn trusted_at(&self, boottime: u64) -> Option<u64> {
        at(boottime, self.offset)
    }

    /// Observed now at `boottime`.
    pub fn observed_at(&self, boottime: u64) -> Option<u64> {
        at(boottime, self.observed_offset)
    }
}

/// A clock anomaly episode: from the first sample more than 300 s behind trusted time
/// until a sample is within 300 s again (erratum E6).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct anomaly_state_t {
    /// Episodes so far in this run; the current one when `open`.
    pub episode: u64,
    pub open: bool,
    /// The largest lag seen in the current or the last episode (this run's samples
    /// only: not persisted).
    pub behind: u64,
}

impl anomaly_state_t {
    /// The persisted form: 0 before any episode, `2·episode` while it is open,
    /// `2·episode + 1` once it closed. Later states have larger marks.
    pub fn mark(&self) -> u64 {
        let base = self.episode.saturating_mul(2);
        if self.episode == 0 || self.open {
            base
        } else {
            base.saturating_add(1)
        }
    }

    /// The state a persisted mark records.
    pub fn from_mark(mark: u64) -> anomaly_state_t {
        anomaly_state_t {
            episode: mark / 2,
            open: mark != 0 && mark % 2 == 0,
            behind: 0,
        }
    }
}

/// One service's trusted time in one boot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct trusted_clock_t {
    boot_id: String,
    offset: i64,
    observed_offset: i64,
    /// The latest CLOCK_BOOTTIME observed: now never goes back to an earlier one.
    last_boottime: u64,
    anomaly: anomaly_state_t,
}

impl trusted_clock_t {
    /// Starts the clock from a sample and the persisted floors (rules 1, 3 and 6;
    /// option B). Floors of this boot: resume from the largest offsets. Floors of
    /// earlier boots only: start no earlier than the last trusted and observed values.
    /// The observed counter of a store without floors starts at zero. The anomaly
    /// episode of this boot is resumed, then the start's own sample is taken into
    /// account like any other (erratum E6; M2-006-B1-R2-01): within 300 s of trusted
    /// time it ends a resumed episode, more than 300 s behind it opens or continues
    /// one. A restart alone never ends an episode. The caller persists the result
    /// before serving (the run marker's anchor write publishes the floor).
    pub fn start(
        boot_id: &str,
        sample: time_sample_t,
        floors: &[time_floor_t],
    ) -> Option<trusted_clock_t> {
        let boottime = signed(sample.boottime)?;
        let mut offset = signed(sample.realtime)?.checked_sub(boottime)?;
        let mut observed_offset = 0i64.checked_sub(boottime)?;
        let same_boot: Vec<&time_floor_t> = floors
            .iter()
            .filter(|floor| floor.boot_id == boot_id)
            .collect();
        // Episodes are counted per boot: a same-boot restart resumes the latest one.
        let anomaly = anomaly_state_t::from_mark(
            same_boot
                .iter()
                .map(|floor| floor.anomaly)
                .max()
                .unwrap_or(0),
        );
        if same_boot.is_empty() {
            let earlier = floors.iter();
            if let Some(last_trusted) = earlier.clone().map(|floor| floor.trusted).max() {
                offset = offset.max(signed(last_trusted)?.checked_sub(boottime)?);
            }
            if let Some(last_observed) = earlier.map(|floor| floor.observed).max() {
                observed_offset =
                    observed_offset.max(signed(last_observed)?.checked_sub(boottime)?);
            }
        } else {
            for floor in same_boot {
                offset = offset.max(floor.offset()?);
                observed_offset = observed_offset.max(floor.observed_offset()?);
            }
        }
        let mut clock = trusted_clock_t {
            boot_id: boot_id.to_owned(),
            offset,
            observed_offset,
            last_boottime: sample.boottime,
            anomaly,
        };
        // The offset already covers the sample's own (it is the start value or
        // larger), so this changes only the episode.
        clock.observe(sample, false);
        Some(clock)
    }

    pub fn boot_id(&self) -> &str {
        &self.boot_id
    }

    pub fn offset(&self) -> i64 {
        self.offset
    }

    /// What `sample` would change (rule 2); nothing changes yet. A sample older than
    /// the latest observation still counts for its own offset.
    pub fn assess(&self, sample: time_sample_t) -> Option<assessment_t> {
        let sample_offset = signed(sample.realtime)?.checked_sub(signed(sample.boottime)?)?;
        let trusted = at(sample.boottime, self.offset)?;
        let raise = (sample_offset > self.offset).then_some(sample_offset);
        let behind = trusted
            .checked_sub(sample.realtime)
            .filter(|behind| *behind > AnomalySeconds);
        Some(assessment_t { raise, behind })
    }

    /// Takes `sample` into account once its raise, if any, is durable: CLOCK_BOOTTIME
    /// advances, the offset rises (`raised` is false when the raise could not be made
    /// durable: nothing relies on it) and the anomaly episode opens or ends. Returns
    /// the episode the sample belongs to when it is anomalous.
    pub fn observe(&mut self, sample: time_sample_t, raised: bool) -> Option<u64> {
        self.last_boottime = self.last_boottime.max(sample.boottime);
        let assessment = self.assess(sample)?;
        if raised {
            if let Some(offset) = assessment.raise {
                self.offset = self.offset.max(offset);
            }
        }
        match assessment.behind {
            Some(behind) => {
                if !self.anomaly.open {
                    self.anomaly.open = true;
                    self.anomaly.episode = self.anomaly.episode.saturating_add(1);
                    self.anomaly.behind = 0;
                }
                self.anomaly.behind = self.anomaly.behind.max(behind);
                Some(self.anomaly.episode)
            }
            None => {
                self.anomaly.open = false;
                None
            }
        }
    }

    /// Takes a peer's offset (rule 4); it only ever rises.
    pub fn adopt(&mut self, offset: i64) {
        self.offset = self.offset.max(offset);
    }

    /// Trusted now, at the latest observed CLOCK_BOOTTIME.
    pub fn now(&self) -> Option<u64> {
        at(self.last_boottime, self.offset)
    }

    /// Observed now, at the latest observed CLOCK_BOOTTIME.
    pub fn observed_now(&self) -> Option<u64> {
        at(self.last_boottime, self.observed_offset)
    }

    /// The latest observed CLOCK_BOOTTIME.
    pub fn boottime(&self) -> u64 {
        self.last_boottime
    }

    /// The floor this clock persists now.
    pub fn floor(&self) -> Option<time_floor_t> {
        Some(time_floor_t {
            boot_id: self.boot_id.clone(),
            boottime: self.last_boottime,
            trusted: self.now()?,
            observed: self.observed_now()?,
            anomaly: self.anomaly.mark(),
        })
    }

    /// The floor a raise to `offset` persists (the ledger entry written before the
    /// raise is adopted).
    pub fn floor_with(&self, offset: i64) -> Option<time_floor_t> {
        Some(time_floor_t {
            boot_id: self.boot_id.clone(),
            boottime: self.last_boottime,
            trusted: at(self.last_boottime, self.offset.max(offset))?,
            observed: self.observed_now()?,
            anomaly: self.anomaly.mark(),
        })
    }

    pub fn view(&self) -> time_view_t {
        time_view_t {
            boot_id: self.boot_id.clone(),
            offset: self.offset,
            observed_offset: self.observed_offset,
        }
    }

    pub fn anomaly(&self) -> anomaly_state_t {
        self.anomaly
    }
}

/// Whether a commit timestamp passes the commit-timestamp rule of §5 against trusted
/// `now` and its parents' timestamps (erratum E4, `timestamp_rule`): within ±300 s
/// of trusted now, and not earlier than any parent's timestamp minus 300 s.
pub fn timestamp_acceptable(timestamp: i64, now: i64, parents: &[i64]) -> bool {
    let skew = i64::try_from(AdmissionSkewSeconds).unwrap_or(0);
    let within = timestamp
        .checked_sub(now)
        .is_some_and(|difference| difference.checked_abs().is_some_and(|abs| abs <= skew));
    let after_parents = parents.iter().all(|parent| {
        parent
            .checked_sub(skew)
            .is_some_and(|earliest| timestamp >= earliest)
    });
    within && after_parents
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(realtime: u64, boottime: u64) -> time_sample_t {
        time_sample_t { realtime, boottime }
    }

    fn floor(boot_id: &str, boottime: u64, trusted: u64, observed: u64) -> time_floor_t {
        time_floor_t {
            boot_id: boot_id.to_owned(),
            boottime,
            trusted,
            observed,
            anomaly: 0,
        }
    }

    #[test]
    fn the_start_sample_ends_or_continues_the_resumed_episode() {
        let open_floor = time_floor_t {
            anomaly: 2,
            ..floor("b", 100, 5_000, 90)
        };
        // Within 300 s of trusted time (5 010 at boottime 110): the episode ends.
        let ended = trusted_clock_t::start("b", sample(4_710, 110), &[open_floor.clone()]).unwrap();
        assert_eq!(ended.anomaly().mark(), 3);
        // More than 300 s behind: the episode continues.
        let continued =
            trusted_clock_t::start("b", sample(4_709, 110), &[open_floor.clone()]).unwrap();
        assert_eq!(continued.anomaly().mark(), 2);
        // After an ended episode, a lag at the start opens the next one.
        let closed_floor = time_floor_t {
            anomaly: 3,
            ..open_floor
        };
        let next = trusted_clock_t::start("b", sample(4_000, 110), &[closed_floor]).unwrap();
        assert_eq!(next.anomaly().mark(), 4);
        // The start's sample is never a raise to adopt: the offset is the start value.
        assert_eq!(ended.now(), Some(5_010));
    }

    #[test]
    fn anomaly_episodes_resume_in_the_same_boot_only() {
        let open = anomaly_state_t {
            episode: 3,
            open: true,
            behind: 400,
        };
        let closed = anomaly_state_t {
            open: false,
            ..open
        };
        assert_eq!(open.mark(), 6);
        assert_eq!(closed.mark(), 7);
        assert_eq!(anomaly_state_t::default().mark(), 0);
        assert_eq!(
            anomaly_state_t::from_mark(6),
            anomaly_state_t { behind: 0, ..open }
        );
        assert_eq!(anomaly_state_t::from_mark(7).episode, 3);
        assert!(!anomaly_state_t::from_mark(7).open);
        assert_eq!(anomaly_state_t::from_mark(0), anomaly_state_t::default());
        // A restart in the same boot resumes the latest state; another boot starts over.
        let floors = [
            time_floor_t {
                anomaly: 6,
                ..floor("b", 100, 5_000, 90)
            },
            time_floor_t {
                anomaly: 4,
                ..floor("b", 120, 5_020, 110)
            },
        ];
        let resumed = trusted_clock_t::start("b", sample(4_000, 130), &floors).unwrap();
        assert_eq!(resumed.anomaly().mark(), 6);
        // Another boot counts its own episodes: its start, 1 020 s behind the last floor,
        // opens episode 1; a start within 300 s of it opens none.
        let rebooted = trusted_clock_t::start("c", sample(4_000, 5), &floors).unwrap();
        assert_eq!(rebooted.anomaly().mark(), 2);
        let settled = trusted_clock_t::start("c", sample(5_020, 5), &floors).unwrap();
        assert_eq!(settled.anomaly().mark(), 0);
        // The resumed episode continues: a sample still behind belongs to it.
        let mut clock = resumed;
        assert_eq!(clock.observe(sample(4_000, 140), true), Some(3));
        assert_eq!(clock.anomaly().episode, 3);
        // Its end is a later state, and the next episode a later one still.
        clock.observe(sample(5_040, 140), true);
        assert_eq!(clock.anomaly().mark(), 7);
        clock.observe(sample(4_000, 150), true);
        assert_eq!(clock.anomaly().mark(), 8);
        assert_eq!(
            merge_floor(Some(floors[0].clone()), floors[1].clone()).anomaly,
            6
        );
    }

    #[test]
    fn rebases_on_forward_samples_and_never_goes_back() {
        let mut clock = trusted_clock_t::start("b", sample(1_000, 10), &[]).unwrap();
        assert_eq!(clock.now(), Some(1_000));
        // Forward step to 4600 at boottime 20, then a correction back to 1010.
        assert_eq!(clock.assess(sample(4_600, 20)).unwrap().raise, Some(4_580));
        clock.observe(sample(4_600, 20), true);
        assert_eq!(clock.now(), Some(4_600));
        let episode = clock.observe(sample(1_010, 30), true);
        assert_eq!(clock.now(), Some(4_610));
        assert_eq!(episode, Some(1));
        assert!(clock.anomaly().open);
        // An hour later the trusted time continues at BOOTTIME pace.
        clock.observe(sample(8_200, 3_630), true);
        assert_eq!(clock.now(), Some(8_210));
        assert!(!clock.anomaly().open);
    }

    #[test]
    fn an_unpersisted_raise_is_not_adopted() {
        let mut clock = trusted_clock_t::start("b", sample(1_000, 10), &[]).unwrap();
        clock.observe(sample(2_000, 20), false);
        assert_eq!(clock.now(), Some(1_010));
    }

    #[test]
    fn same_boot_restart_resumes_from_the_largest_floor() {
        let floors = [floor("b", 100, 5_000, 90), floor("b", 50, 4_000, 60)];
        let clock = trusted_clock_t::start("b", sample(1_000, 120), &floors).unwrap();
        assert_eq!(clock.now(), Some(5_020));
        // The observed offsets are 10 and −10: the larger one wins.
        assert_eq!(clock.observed_now(), Some(130));
    }

    #[test]
    fn a_new_boot_starts_no_earlier_than_the_last_floor() {
        let floors = [floor("a", 100, 5_000, 400)];
        let clock = trusted_clock_t::start("b", sample(1_000, 5), &floors).unwrap();
        assert_eq!(clock.now(), Some(5_000));
        // Powered-off time never counts: observed resumes from the last value.
        assert_eq!(clock.observed_now(), Some(400));
        let ahead = trusted_clock_t::start("b", sample(9_000, 5), &floors).unwrap();
        assert_eq!(ahead.now(), Some(9_000));
    }

    #[test]
    fn merged_floors_never_regress() {
        let late = floor("b", 200, 5_200, 160);
        let early = floor("b", 100, 4_000, 60);
        let merged = merge_floor(Some(late.clone()), early);
        assert_eq!(merged, late);
        // A concurrent writer that read the clock earlier cannot lower either value.
        let merged = merge_floor(
            Some(floor("b", 150, 5_100, 110)),
            floor("b", 120, 5_200, 80),
        );
        assert_eq!(merged, floor("b", 150, 5_230, 110));
        let other_boot = merge_floor(Some(late), floor("c", 5, 5_300, 165));
        assert_eq!(other_boot.boot_id, "c");
    }

    #[test]
    fn the_kernel_source_reads_both_clocks() {
        let first = kernel_time_source_t.sample().expect("kernel clocks");
        let second = kernel_time_source_t.sample().expect("kernel clocks");
        // 2020-01-01T00:00:00Z: the wall clock is set; CLOCK_BOOTTIME never decreases.
        assert!(first.realtime > 1_577_836_800);
        assert!(second.boottime >= first.boottime);
        assert!(second.realtime + 1 >= first.realtime);
    }

    #[test]
    fn timestamp_rule_bounds() {
        assert!(timestamp_acceptable(1_300, 1_000, &[]));
        assert!(timestamp_acceptable(700, 1_000, &[]));
        assert!(!timestamp_acceptable(1_301, 1_000, &[]));
        assert!(!timestamp_acceptable(699, 1_000, &[]));
        assert!(timestamp_acceptable(1_000, 1_000, &[1_300]));
        assert!(!timestamp_acceptable(1_000, 1_000, &[1_301]));
    }
}
