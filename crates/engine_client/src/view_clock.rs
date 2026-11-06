//! How far behind the server others are drawn (proposal 0048).

use engine_core::time::{MOST_VIEW_DELAY, TICK_SECONDS};

/// How far behind the newest snapshot to draw at least, in seconds: two ticks, which
/// covers one lost snapshot and no jitter.
const LEAST_DELAY: f64 = 2.0 * TICK_SECONDS;

/// How much of the longest wait is kept at each snapshot. It halves in about six seconds at
/// 72 a second, so a bad moment is remembered, and a better connection is noticed.
const WAIT_KEPT: f64 = 0.9985;

/// How much the clock can run fast or slow to stay on course.
const MAX_STRETCH: f64 = 0.1;

/// How quickly it closes a gap, as a share of the gap each second.
const CATCH_UP: f64 = 2.0;

/// Further out than this, it jumps rather than stretches.
const MAX_GAP: f64 = 0.25;

/// A steady clock that follows snapshots arriving unevenly, so what's drawn
/// moves evenly.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewClock {
    /// The time being drawn.
    time: f64,
    /// Whether it's been set.
    started: bool,
    /// The newest snapshot seen.
    newest: f64,
    /// How long since it came.
    since_newest: f64,
    /// The longest a snapshot's been waited for lately, past its tick.
    longest_wait: f64,
}

impl ViewClock {
    /// Moves on by `seconds`, with `newest` the latest snapshot's time, and
    /// returns the time to draw.
    pub fn advance(&mut self, seconds: f64, newest: f64) -> f64 {
        self.note_arrival(seconds, newest);
        let wanted = newest - self.delay();
        let gap = wanted - self.time;
        if !self.started || gap.abs() > MAX_GAP {
            self.started = true;
            self.time = wanted;
            return self.time;
        }
        let stretch = (gap * CATCH_UP).clamp(-MAX_STRETCH, MAX_STRETCH);
        self.time += seconds * (1.0 + stretch);
        // Drawing past the newest snapshot would have to guess.
        self.time = self.time.min(newest);
        self.time
    }

    /// Keeps track of how long the gaps between snapshots are. Jitter and lost snapshots
    /// come to the same thing here: a wait with nothing new to draw towards.
    fn note_arrival(&mut self, seconds: f64, newest: f64) {
        self.since_newest += seconds;
        if newest > self.newest {
            let wait = (self.since_newest - TICK_SECONDS).max(0.0);
            self.longest_wait = (self.longest_wait * WAIT_KEPT).max(wait);
            self.newest = newest;
            self.since_newest = 0.0;
        }
    }

    /// How far behind the newest snapshot to draw: long enough to ride out the worst wait
    /// lately, and no longer.
    fn delay(&self) -> f64 {
        (TICK_SECONDS + self.longest_wait).clamp(LEAST_DELAY, MOST_VIEW_DELAY)
    }

    /// The time last drawn.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.time
    }
}
