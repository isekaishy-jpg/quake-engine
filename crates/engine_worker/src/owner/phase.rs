//! Application-defined phase eligibility and bounded pumping.

use std::time::Duration;

/// A host-defined publication phase. The host chooses its values and advances
/// the owner explicitly; this crate does not impose game or frame event IDs.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Phase(pub u64);

/// Whether a pump sees newly eligible callbacks during the same call.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PumpMode {
    /// Recheck the route after each callback and local cleanup.
    #[default]
    Live,
    /// Freeze the eligible frontier at pump entry. Closure or fault can suppress
    /// those deliveries, but does not add other phases to this batch's cleanup.
    Batch,
}

/// Host-chosen limit for one route pump. Limits are checked between entries,
/// including suppressed callbacks needing local cleanup. A single callback or
/// destructor may exceed the time limit. A zero count performs no work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PumpBudget {
    /// The most entries to deal with.
    pub max_entries: usize,
    /// The longest to spend, if there's a limit.
    pub max_duration: Option<Duration>,
    /// How entries are dealt with.
    pub mode: PumpMode,
}

impl PumpBudget {
    /// A budget of `max_entries`, with no limit on time.
    pub fn new(max_entries: usize) -> Self {
        Self {
            max_entries,
            max_duration: None,
            mode: PumpMode::Live,
        }
    }

    /// This budget, with a limit on time.
    pub fn with_duration(mut self, duration: Duration) -> Self {
        self.max_duration = Some(duration);
        self
    }

    /// This budget, with its mode.
    pub fn with_mode(mut self, mode: PumpMode) -> Self {
        self.mode = mode;
        self
    }
}

/// Work completed by one bounded route pump.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PumpReport {
    /// How many were dealt with.
    pub invoked: usize,
    /// How many were dropped.
    pub suppressed: usize,
}

impl PumpReport {
    /// How many were looked at, either way.
    pub fn processed(self) -> usize {
        self.invoked + self.suppressed
    }
}
