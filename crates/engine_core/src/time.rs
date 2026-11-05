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
        self.fraction_after(Duration::ZERO)
    }

    /// The fraction `extra` after the clock last advanced, stopping at 1.
    /// A frame uses this, because the clock only advances when ticks run, so
    /// between ticks [`Self::fraction`] stands still (proposal 0010).
    #[must_use]
    pub fn fraction_after(&self, extra: Duration) -> f64 {
        let start = self.nanos_for(u128::from(self.tick.0));
        let end = self.nanos_for(u128::from(self.tick.0) + 1);
        let into = (self.elapsed_nanos + extra.as_nanos()).saturating_sub(start);
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

/// The shortest time between readings, which is 4 a second.
const FPS_INTERVAL: Duration = Duration::from_millis(250);

/// How quickly readings follow a change in frame rate, in seconds.
const FPS_TIME_CONSTANT: f64 = 1.0;

/// A smoothed frame rate that's steady enough to read (proposal 0021).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FpsMeter {
    /// Frames since the last reading.
    frames: u32,
    /// Time since the last reading.
    elapsed: Duration,
    /// The smoothed rate, once there is one.
    smoothed: Option<f64>,
}

impl FpsMeter {
    /// A meter with no readings yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Counts a frame that took `delta`. Returns a new reading, at most every
    /// 250 ms, in whole frames a second.
    pub fn frame(&mut self, delta: Duration) -> Option<u32> {
        self.frames += 1;
        self.elapsed += delta;
        if self.elapsed < FPS_INTERVAL {
            return None;
        }
        let seconds = self.elapsed.as_secs_f64();
        let rate = f64::from(self.frames) / seconds;
        let weight = 1.0 - (-seconds / FPS_TIME_CONSTANT).exp();
        let smoothed = self
            .smoothed
            .map_or(rate, |previous| previous + (rate - previous) * weight);
        self.smoothed = Some(smoothed);
        self.frames = 0;
        self.elapsed = Duration::ZERO;
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a frame rate is small and never negative"
        )]
        Some(smoothed.round() as u32)
    }

    /// Forgets everything, for after a gap in frames.
    pub fn reset(&mut self) {
        *self = Self::new();
    }
}

/// How many ticks a second the game runs at (proposal 0010).
pub const TICK_HERTZ: u32 = 72;

/// The length of a tick.
pub const TICK_SECONDS: f64 = 1.0 / TICK_HERTZ as f64;
