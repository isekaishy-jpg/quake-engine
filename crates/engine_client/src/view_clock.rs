//! How far behind the server others are drawn (proposal 0048).

use engine_core::time::TICK_SECONDS;

/// How far behind the newest snapshot to draw, in seconds: two ticks.
const DELAY: f64 = 2.0 * TICK_SECONDS;

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
}

impl ViewClock {
    /// Moves on by `seconds`, with `newest` the latest snapshot's time, and
    /// returns the time to draw.
    pub fn advance(&mut self, seconds: f64, newest: f64) -> f64 {
        let wanted = newest - DELAY;
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

    /// The time last drawn.
    #[must_use]
    pub fn time(&self) -> f64 {
        self.time
    }
}
