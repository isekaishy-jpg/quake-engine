//! Optional configuration planning; applications retain engine_worker runtimes.

use engine_worker::{RuntimeConfig, ThreadPriority, WorkerConfig};

use crate::CpuTopology;

/// Explicit inputs to independent physical-core class caps, in Low/Mid/High
/// order. There are no automatic reserved-core or class-count defaults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerRequest {
    /// Physical cores subtracted before the per-class ceiling is clamped.
    pub reserved_physical_cores: usize,
    /// Requested class limits. Zero disables that class; nonzero values are
    /// clamped independently between two and the computed ceiling.
    pub class_limits: [usize; 3],
    /// Optional OS priority request for each class, passed directly to `engine_worker`.
    pub priorities: [Option<ThreadPriority>; 3],
}

/// An explicit configuration calculation, not a started runtime or performance
/// recommendation. The application constructs `engine_worker` and handles startup failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerPlan {
    requested: WorkerRequest,
    physical_cores: usize,
    class_ceiling: usize,
    effective: RuntimeConfig,
}

impl WorkerPlan {
    /// Applies the selected independent-class physical-core policy:
    /// `ceiling = clamp(physical cores - reserved cores, 2, 64)`; each nonzero
    /// request is clamped to `[2, ceiling]`, while zero remains disabled.
    ///
    /// Subtraction saturates before clamping. The aggregate `engine_worker` budget equals
    /// the sum of effective class counts; it can exceed the number of physical
    /// or logical processors. This function installs no affinity, starts no
    /// workers and does not override application-chosen direct `engine_worker` configuration.
    ///
    /// # Panics
    ///
    /// Panics if the budget it works out doesn't match its classes, which is a bug in this
    /// crate.
    pub fn independent_physical_caps(topology: &CpuTopology, request: WorkerRequest) -> Self {
        let physical_cores = topology.physical_cores();
        let class_ceiling = physical_cores
            .saturating_sub(request.reserved_physical_cores)
            .clamp(2, 64);
        let classes = std::array::from_fn(|index| {
            let requested = request.class_limits[index];
            let count = if requested == 0 {
                0
            } else {
                requested.clamp(2, class_ceiling)
            };
            let config = WorkerConfig::new(count);
            match request.priorities[index] {
                Some(priority) => config.with_priority(priority),
                None => config,
            }
        });
        // Three independently bounded counts have a sum at most 192.
        let budget = classes.iter().map(|class| class.worker_count()).sum();
        let effective =
            RuntimeConfig::new(budget, classes).expect("bounded class sum is its aggregate budget");
        Self {
            requested: request,
            physical_cores,
            class_ceiling,
            effective,
        }
    }

    /// Unmodified application inputs, including priorities and disabled classes.
    pub fn requested(self) -> WorkerRequest {
        self.requested
    }
    /// Observed physical-core count used for this calculation.
    pub fn physical_cores(self) -> usize {
        self.physical_cores
    }
    /// Independent upper bound used for every nonzero class request.
    pub fn class_ceiling(self) -> usize {
        self.class_ceiling
    }
    /// Valid `engine_worker` configuration; successful runtime construction is still required.
    pub fn effective(self) -> RuntimeConfig {
        self.effective
    }
}

#[cfg(test)]
#[path = "../tests/unit/worker.rs"]
mod tests;
