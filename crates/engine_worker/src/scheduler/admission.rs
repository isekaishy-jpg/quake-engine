//! Bounded owned metadata and runnable admission.

use crate::execution::Group;
use crate::runtime::config::ExecutionClass;
use crate::task::{Completion, ProducerControl, Task};

/// An accepted task and explicit cancellation authority, or the untouched
/// operation with its admission failure.
pub type SpawnResult<T, F> = Result<(Task<T>, ProducerControl), SpawnRejected<F>>;

/// Common execution and dependency constraints for an owned admission batch.
/// A supplied group stays open until its caller seals it.
#[derive(Clone, Copy)]
pub struct BatchSpawnOptions<'a> {
    /// How each is spawned.
    pub spawn: SpawnOptions,
    /// The group they join, if any.
    pub group: Option<&'a Group>,
    /// What has to finish first.
    pub prerequisites: &'a [Completion],
    /// How what comes first decides whether they run.
    pub dependency_policy: DependencyPolicy,
}

impl Default for BatchSpawnOptions<'_> {
    fn default() -> Self {
        Self {
            spawn: SpawnOptions::default(),
            group: None,
            prerequisites: &[],
            dependency_policy: DependencyPolicy::SuccessOnly,
        }
    }
}

impl std::fmt::Debug for BatchSpawnOptions<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BatchSpawnOptions")
            .field("spawn", &self.spawn)
            .field("group", &self.group.as_ref().map(|group| group.class()))
            .field("prerequisites", &self.prerequisites.len())
            .field("dependency_policy", &self.dependency_policy)
            .finish()
    }
}

/// Ordered accepted handles, or an accepted prefix and untouched input suffix.
pub type BatchSpawnResult<'a, T, F> =
    Result<Vec<(Task<T>, ProducerControl)>, BatchSpawnRejected<'a, T, F>>;

/// Admission stopped at the first member in `remaining`. Earlier accepted work
/// may already be executing; retry only the returned suffix. Dropping this
/// receipt does not cancel accepted work.
#[must_use]
pub struct BatchSpawnRejected<'a, T: Send + 'static, F> {
    /// Why the rest weren't taken.
    pub reason: SpawnError,
    /// What was taken.
    pub accepted: Vec<(Task<T>, ProducerControl)>,
    /// What wasn't, handed back.
    pub remaining: Vec<F>,
    /// The options, handed back.
    pub options: BatchSpawnOptions<'a>,
}

impl<T: Send + 'static, F> std::fmt::Debug for BatchSpawnRejected<'_, T, F> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BatchSpawnRejected")
            .field("reason", &self.reason)
            .field("accepted", &self.accepted.len())
            .field("remaining", &self.remaining.len())
            .field("options", &self.options)
            .finish()
    }
}

/// Independent owned-work capacities. Array entries are Low, Mid, High.
/// `records` counts waiting, ready, handed, running, and finalizing jobs;
/// `edges` counts registered prerequisites through terminal detachment.
/// `runnable` counts ready and handed jobs, excluding running jobs, and reserves
/// slots for accepted immediate batch members while their attachment closes.
/// `handoff` counts wrappers offered to the backend until they return,
/// including a wrapper whose job is running. Zero edges disables dependent
/// submissions. Saturated caller execution can exceed only the runnable
/// limit; it still needs record and edge credits.
/// Internal reuse caches retain at most `records` job, group, and completion
/// allocations each, plus `edges` slots of detached subscription buffers.
/// Job and completion caches contain returned controls; groups retire when the
/// last public handle drops after sealing and reuse waits for settlement.
/// Outstanding handles prevent reuse; retained payloads keep their own lifetime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnedLimits {
    pub(crate) records: usize,
    pub(crate) edges: usize,
    pub(crate) runnable: [usize; 3],
    pub(crate) handoff: [usize; 3],
}

impl OwnedLimits {
    /// Limits with each of these counts.
    ///
    /// # Errors
    ///
    /// Returns an error if any count is zero.
    pub fn new(
        records: usize,
        edges: usize,
        runnable: [usize; 3],
        handoff: [usize; 3],
    ) -> Result<Self, OwnedConfigError> {
        if records == 0 {
            return Err(OwnedConfigError::ZeroRecords);
        }
        for class in ExecutionClass::ALL {
            if runnable[class.index()] == 0 {
                return Err(OwnedConfigError::ZeroRunnable(class));
            }
            if handoff[class.index()] == 0 {
                return Err(OwnedConfigError::ZeroHandoff(class));
            }
        }
        Ok(Self {
            records,
            edges,
            runnable,
            handoff,
        })
    }

    pub(crate) fn runnable_for(self, class: ExecutionClass) -> usize {
        self.runnable[class.index()]
    }

    pub(crate) fn handoff_for(self, class: ExecutionClass) -> usize {
        self.handoff[class.index()]
    }
}

/// Why limits on owned work don't add up.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedConfigError {
    /// There's no room for records.
    ZeroRecords,
    /// There's no room for ready work in this class.
    ZeroRunnable(ExecutionClass),
    /// There's no room for handoffs in this class.
    ZeroHandoff(ExecutionClass),
}

impl std::fmt::Display for OwnedConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroRecords => formatter.write_str("owned record capacity must be nonzero"),
            Self::ZeroRunnable(class) => {
                write!(formatter, "{class:?} runnable capacity must be nonzero")
            }
            Self::ZeroHandoff(class) => {
                write!(formatter, "{class:?} handoff capacity must be nonzero")
            }
        }
    }
}

impl std::error::Error for OwnedConfigError {}

/// Why an owned job could not be admitted. Rejected inputs remain with caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpawnError {
    /// This execution class has no workers. Rejected inputs remain uninvoked.
    ClassDisabled(ExecutionClass),
    /// There's no room just now.
    Full,
    /// It could never fit.
    TooLarge,
    /// The runtime has stopped taking work.
    Closed,
    /// What it needs is turned off.
    Disabled,
    /// It can't be done from here.
    InvalidContext,
    /// The group isn't one it can join.
    InvalidGroup,
    /// The promised delivery belongs to another runtime or was already used.
    InvalidDelivery,
    /// The priority isn't known.
    InvalidPriority,
    /// The reservation doesn't fit it.
    InvalidReservation,
    /// What it needs has already been used.
    Consumed,
}

/// Whether a retained job may run on an explicitly participating caller.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallerEligibility {
    /// Only a worker may run it.
    WorkerOnly,
    /// The caller may run it too.
    CallerEligible,
}

/// Whether prerequisite failure suppresses a successor's closure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DependencyPolicy {
    /// It runs only if what comes first succeeded.
    SuccessOnly,
    /// It runs whatever became of what comes first.
    OutcomeAware,
}

/// Execution constraints for an owned job.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SpawnOptions {
    /// Who may run it.
    pub eligibility: CallerEligibility,
}

impl Default for SpawnOptions {
    fn default() -> Self {
        Self {
            eligibility: CallerEligibility::WorkerOnly,
        }
    }
}

/// An owned admission rejection, preserving the uninvoked closure and options.
pub struct SpawnRejected<F> {
    /// Why it wasn't taken.
    pub reason: SpawnError,
    /// The work, handed back.
    pub operation: F,
    /// The options, handed back.
    pub options: SpawnOptions,
}

impl<F> std::fmt::Debug for SpawnRejected<F> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SpawnRejected")
            .field("reason", &self.reason)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}
