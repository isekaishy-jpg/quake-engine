//! Host-supplied worker allocation for the three execution classes.

use std::error::Error;
use std::fmt;

/// The capacity class of CPU work. Classes have separate workers and queues.
///
/// An execution class does not express resource urgency or an operating-system
/// thread priority.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ExecutionClass {
    /// For what can wait.
    Low,
    /// For what's usual.
    Mid,
    /// For what can't wait.
    High,
}

impl ExecutionClass {
    /// The fixed order of class entries in [`RuntimeConfig`].
    pub const ALL: [Self; 3] = [Self::Low, Self::Mid, Self::High];

    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Low => 0,
            Self::Mid => 1,
            Self::High => 2,
        }
    }
}

/// A requested operating-system priority for workers in one class.
///
/// These values correspond to relative priorities -1, 0 and +1 on Windows.
/// Priority setup is fallible; a platform that cannot apply a request rejects
/// runtime construction rather than silently ignoring it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreadPriority {
    /// Below the usual priority.
    BelowNormal,
    /// The usual priority.
    Normal,
    /// Above the usual priority.
    AboveNormal,
}

/// Host configuration for the workers of one execution class.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerConfig {
    worker_count: usize,
    requested_priority: Option<ThreadPriority>,
}

impl WorkerConfig {
    /// Configures a class with the given count and no OS priority request.
    ///
    /// Zero disables the class; lane operations reject work without invoking it.
    /// The count is checked when the complete runtime configuration is built.
    pub const fn new(worker_count: usize) -> Self {
        Self {
            worker_count,
            requested_priority: None,
        }
    }

    /// Requests an OS priority for every worker in this class.
    pub const fn with_priority(mut self, priority: ThreadPriority) -> Self {
        self.requested_priority = Some(priority);
        self
    }

    /// How many workers it has.
    pub const fn worker_count(self) -> usize {
        self.worker_count
    }

    /// The priority it asks for, if any.
    pub const fn requested_priority(self) -> Option<ThreadPriority> {
        self.requested_priority
    }
}

/// A validated, host-supplied aggregate worker budget and Low/Mid/High split.
///
/// A zero count disables a class. Its execution and owned-admission APIs reject
/// work without invoking it. Enabled classes have a worker route; their counts
/// may use less than the aggregate budget. An all-disabled runtime is permitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeConfig {
    worker_budget: usize,
    classes: [WorkerConfig; 3],
}

impl RuntimeConfig {
    /// Validates the aggregate budget and class split before worker startup.
    ///
    /// # Errors
    ///
    /// Returns an error if the budget is zero, or the classes ask for more than it has.
    pub fn new(worker_budget: usize, classes: [WorkerConfig; 3]) -> Result<Self, ConfigError> {
        let configured = classes[0]
            .worker_count
            .checked_add(classes[1].worker_count)
            .and_then(|count| count.checked_add(classes[2].worker_count))
            .ok_or(ConfigError::WorkerCountOverflow)?;
        if worker_budget == 0 && configured != 0 {
            return Err(ConfigError::ZeroBudget);
        }
        if configured > worker_budget {
            return Err(ConfigError::BudgetExceeded {
                budget: worker_budget,
                configured,
            });
        }

        Ok(Self {
            worker_budget,
            classes,
        })
    }

    /// How many workers there can be.
    pub const fn worker_budget(&self) -> usize {
        self.worker_budget
    }

    /// How the workers of `class` are set up.
    pub const fn workers_for(&self, class: ExecutionClass) -> WorkerConfig {
        self.classes[class.index()]
    }
}

/// Invalid aggregate worker allocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConfigError {
    /// There are no workers to give out.
    ZeroBudget,
    /// Retained for source compatibility; zero now disables a class.
    ZeroWorkers(ExecutionClass),
    /// The count of workers overflowed.
    WorkerCountOverflow,
    /// More workers were asked for than the budget has.
    BudgetExceeded {
        /// How many there can be.
        budget: usize,
        /// How many were asked for.
        configured: usize,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroBudget => {
                formatter.write_str("enabled classes require a nonzero worker budget")
            }
            Self::ZeroWorkers(class) => write!(formatter, "{class:?} class has no workers"),
            Self::WorkerCountOverflow => formatter.write_str("worker counts overflow usize"),
            Self::BudgetExceeded { budget, configured } => write!(
                formatter,
                "worker budget is {budget}, but class counts require {configured}"
            ),
        }
    }
}

impl Error for ConfigError {}
