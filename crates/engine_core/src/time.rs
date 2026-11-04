//! Simulation time: a fixed tick rate driven by elapsed real time.
//!
//! Ticks are counted from total elapsed time with integer arithmetic, so a
//! rate that doesn't divide a second evenly never drifts (proposal 0010).

use std::time::Duration;

/// Nanoseconds in one second.
const NANOS_PER_SECOND: u128 = 1_000_000_000;

/// A count of simulation ticks since the clock started.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tick(pub u64);

/// How many simulation ticks run in one second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TickRate {
    /// Ticks per second, never zero.
    hertz: u32,
}

impl TickRate {
    /// A rate of `hertz` ticks per second, or `None` for zero.
    #[must_use]
    pub const fn new(hertz: u32) -> Option<Self> {
        if hertz == 0 {
            None
        } else {
            Some(Self { hertz })
        }
    }

    /// Ticks per second.
    #[must_use]
    pub const fn hertz(self) -> u32 {
        self.hertz
    }
}

/// Turns elapsed real time into a count of due ticks.
#[derive(Clone, Debug)]
pub struct TickClock {
    /// The tick rate.
    rate: TickRate,
    /// Elapsed nanoseconds that count towards ticks. Dropped time is removed.
    elapsed_nanos: u128,
    /// Ticks already handed out.
    tick: Tick,
    /// The most ticks one call may hand out.
    max_catch_up: u64,
}

impl TickClock {
    /// A clock at `rate` that hands out at most `max_catch_up` of time per
    /// call. Time beyond that is dropped, so a stall can't start a catch-up
    /// spiral.
    #[must_use]
    pub fn new(rate: TickRate, max_catch_up: Duration) -> Self {
        let nanos = max_catch_up.as_nanos();
        let ticks = nanos * u128::from(rate.hertz()) / NANOS_PER_SECOND;
        Self {
            rate,
            elapsed_nanos: 0,
            tick: Tick(0),
            max_catch_up: u64::try_from(ticks).unwrap_or(u64::MAX).max(1),
        }
    }

    /// The tick rate.
    #[must_use]
    pub fn rate(&self) -> TickRate {
        self.rate
    }

    /// The number of ticks handed out so far.
    #[must_use]
    pub fn tick(&self) -> Tick {
        self.tick
    }

    /// Ticks owed for the total elapsed time, including those handed out.
    fn ticks_owed(&self) -> u128 {
        self.elapsed_nanos * u128::from(self.rate.hertz()) / NANOS_PER_SECOND
    }

    /// The elapsed nanoseconds at which `ticks` ticks are owed, rounded up.
    fn nanos_for(&self, ticks: u128) -> u128 {
        (ticks * NANOS_PER_SECOND).div_ceil(u128::from(self.rate.hertz()))
    }

    /// Adds `elapsed` and returns how many ticks to run now.
    pub fn advance(&mut self, elapsed: Duration) -> u64 {
        self.elapsed_nanos += elapsed.as_nanos();
        let handed_out = u128::from(self.tick.0);
        let due = self.ticks_owed() - handed_out;
        let limit = u128::from(self.max_catch_up);
        if due > limit {
            // Drop the excess, keeping the part of the current tick.
            let dropped = due - limit;
            self.elapsed_nanos -= self.nanos_for(dropped);
        }
        let due = (self.ticks_owed() - handed_out).min(limit);
        let count = u64::try_from(due).unwrap_or(self.max_catch_up);
        self.tick.0 += count;
        count
    }

    /// How far time is into the next tick, from 0 up to, but not including, 1.
    #[must_use]
    pub fn fraction(&self) -> f64 {
        let start = self.nanos_for(u128::from(self.tick.0));
        let end = self.nanos_for(u128::from(self.tick.0) + 1);
        let into = self.elapsed_nanos.saturating_sub(start);
        let length = end - start;
        #[expect(
            clippy::cast_precision_loss,
            reason = "both values are shorter than one tick, far below the precision limit"
        )]
        let fraction = into as f64 / length as f64;
        fraction.clamp(0.0, 1.0)
    }

    /// The time until the next tick is due.
    #[must_use]
    pub fn until_next(&self) -> Duration {
        let next = self.nanos_for(u128::from(self.tick.0) + 1);
        let nanos = next.saturating_sub(self.elapsed_nanos);
        Duration::from_nanos(u64::try_from(nanos).unwrap_or(u64::MAX))
    }
}
