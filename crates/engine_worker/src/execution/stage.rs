//! Composition of lifetime, resource demand, delivery and declared capacity.

use super::{Group, Lane};
use crate::owner::DeliveryTicket;
use crate::scheduler::{
    Cost, DependencyPolicy, DiscoveryPermit, Priority, Reservation, SpawnError, SpawnOptions,
    WorkSet,
};
use crate::task::{Completion, ProducerControl, Retained, Task};

/// Optional policies for a resource stage. Ordinary `try_spawn` remains available
/// without pipeline prediction or declared bytes. Rejection preserves this bundle.
pub struct StageOptions<'a> {
    /// How it's spawned.
    pub spawn: SpawnOptions,
    /// The group it joins, if any.
    pub group: Option<&'a Group>,
    /// What has to finish first.
    pub prerequisites: &'a [Completion],
    /// How what comes first decides whether it runs.
    pub policy: DependencyPolicy,
    /// The work set it counts towards, if any.
    pub work_set: Option<&'a WorkSet>,
    /// What lets it join a sealed work set, if anything.
    pub discovery: Option<&'a DiscoveryPermit>,
    /// How much it's wanted, if that's known.
    pub priority: Option<Priority>,
    /// The capacity set aside for it, if any.
    pub reservation: Option<&'a Reservation>,
    /// What it's expected to cost.
    pub cost: Cost,
    /// How many bytes its result keeps.
    pub retained_bytes: usize,
    /// The delivery it finishes with, if any.
    pub delivery: Option<DeliveryTicket>,
}

impl Default for StageOptions<'_> {
    fn default() -> Self {
        Self {
            spawn: SpawnOptions::default(),
            group: None,
            prerequisites: &[],
            policy: DependencyPolicy::SuccessOnly,
            work_set: None,
            discovery: None,
            priority: None,
            reservation: None,
            cost: Cost::default(),
            retained_bytes: 0,
            delivery: None,
        }
    }
}

/// A stage that wasn't taken.
pub struct StageRejected<'a, F> {
    /// Why it wasn't taken.
    pub reason: SpawnError,
    /// The work, handed back.
    pub operation: F,
    /// The options, handed back.
    pub options: StageOptions<'a>,
}

impl<F> std::fmt::Debug for StageRejected<'_, F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StageRejected")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

/// What spawning a stage gives.
pub type StageResult<'a, T, F> = Result<(Task<Retained<T>>, ProducerControl), StageRejected<'a, F>>;

impl Lane {
    /// Admits a resource stage without executing it on the caller. Declared
    /// retained bytes follow the returned value even after set/runtime drain.
    ///
    /// # Errors
    ///
    /// Hands the work back, with why, if it isn't taken.
    pub fn try_spawn_stage<'a, F, T>(
        &self,
        options: StageOptions<'a>,
        operation: F,
    ) -> StageResult<'a, T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.submit_stage(options, operation, |_| false, false)
    }

    /// Spawns a stage that can fail, or hands it back.
    ///
    /// # Errors
    ///
    /// Hands the work back, with why, if it isn't taken.
    pub fn try_spawn_fallible_stage<'a, F, T, E>(
        &self,
        options: StageOptions<'a>,
        operation: F,
    ) -> StageResult<'a, Result<T, E>, F>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        self.submit_stage(options, operation, |result| result.is_err(), false)
    }

    /// Saturated caller execution still requires explicit caller eligibility.
    ///
    /// # Errors
    ///
    /// Hands the work back, with why, if it can't be taken or run here.
    pub fn submit_or_run_stage<'a, F, T>(
        &self,
        options: StageOptions<'a>,
        operation: F,
    ) -> StageResult<'a, T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.submit_stage(options, operation, |_| false, true)
    }

    /// Spawns a stage that can fail, or runs it here if there's no room.
    ///
    /// # Errors
    ///
    /// Hands the work back, with why, if it can't be taken or run here.
    pub fn submit_or_run_fallible_stage<'a, F, T, E>(
        &self,
        options: StageOptions<'a>,
        operation: F,
    ) -> StageResult<'a, Result<T, E>, F>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        self.submit_stage(options, operation, |result| result.is_err(), true)
    }

    fn submit_stage<'a, F, T>(
        &self,
        mut options: StageOptions<'a>,
        operation: F,
        failed: fn(&Retained<T>) -> bool,
        allow_inline: bool,
    ) -> StageResult<'a, T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let scheduler = match self.control.owned_scheduler_for(self.class) {
            Ok(scheduler) => scheduler,
            Err(reason) => {
                return Err(StageRejected {
                    reason,
                    operation,
                    options,
                });
            }
        };
        let prepared = scheduler.prepare_stage(self.control.identity(), &options);
        let (extras, bytes) = match prepared {
            Ok(prepared) => prepared,
            Err(reason) => {
                return Err(StageRejected {
                    reason,
                    operation,
                    options,
                });
            }
        };
        let request = crate::scheduler::SubmitRequest {
            control: &self.control,
            class: self.class,
            group: options.group,
            options: options.spawn,
            prerequisites: options.prerequisites,
            policy: options.policy,
            allow_inline,
        };
        let result = scheduler.submit_payload_accounted(
            request,
            (operation, bytes),
            run_stage::<F, T>,
            failed,
            &mut options.delivery,
            extras,
        );
        result.map_err(|rejected| {
            let (operation, _bytes) = rejected.operation;
            StageRejected {
                reason: rejected.reason,
                operation,
                options,
            }
        })
    }
}

fn run_stage<F, T>(
    (operation, bytes): (F, Option<crate::scheduler::reservation::ByteLease>),
) -> Retained<T>
where
    F: FnOnce() -> T,
{
    Retained::new(operation(), bytes)
}
