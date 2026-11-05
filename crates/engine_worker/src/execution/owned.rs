//! Owned admission through runtime-associated lanes.

use super::{Group, Lane};
use crate::scheduler::{
    BatchSpawnOptions, BatchSpawnRejected, BatchSpawnResult, DependencyPolicy, SpawnError,
    SpawnOptions, SpawnRejected, SpawnResult, SubmitRequest, call_once, never_fail, result_is_err,
};
use crate::task::Completion;

struct OwnedRoute<'a> {
    options: SpawnOptions,
    group: Option<&'a Group>,
    prerequisites: &'a [Completion],
    policy: DependencyPolicy,
    allow_inline: bool,
}

impl OwnedRoute<'_> {
    fn new(options: SpawnOptions) -> Self {
        Self {
            options,
            group: None,
            prerequisites: &[],
            policy: DependencyPolicy::SuccessOnly,
            allow_inline: false,
        }
    }
}

impl Lane {
    /// Admits a finite owned wave in bounded portions, preserving input order.
    /// Rejection returns accepted handles and every untouched suffix operation.
    /// Workers may begin before return; this method never runs jobs inline as a
    /// saturation fallback and never seals the supplied group. Empty input is a
    /// no-op, including on closed or owned-disabled runtimes.
    pub fn try_spawn_batch<'a, F, T>(
        &self,
        options: BatchSpawnOptions<'a>,
        operations: Vec<F>,
    ) -> BatchSpawnResult<'a, T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.submit_batch(options, operations, call_once::<F, T>, never_fail::<T>)
    }

    /// Admits an owned wave with typed application failures. Returned `Err`
    /// values mark failure for success-only dependents without losing the typed
    /// result. Admission, suffix recovery and caller eligibility match
    /// [`Self::try_spawn_batch`].
    pub fn try_spawn_batch_fallible<'a, F, T, E>(
        &self,
        options: BatchSpawnOptions<'a>,
        operations: Vec<F>,
    ) -> BatchSpawnResult<'a, Result<T, E>, F>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        self.submit_batch(
            options,
            operations,
            call_once::<F, Result<T, E>>,
            result_is_err::<T, E>,
        )
    }

    fn submit_batch<'a, P: Send + 'static, T: Send + 'static>(
        &self,
        options: BatchSpawnOptions<'a>,
        operations: Vec<P>,
        run: fn(P) -> T,
        application_failed: fn(&T) -> bool,
    ) -> BatchSpawnResult<'a, T, P> {
        if operations.is_empty() {
            return Ok(Vec::new());
        }
        let scheduler = match self.control.owned_scheduler_for(self.class) {
            Ok(scheduler) => scheduler,
            Err(reason) => {
                return Err(BatchSpawnRejected {
                    reason,
                    accepted: Vec::new(),
                    remaining: operations,
                    options,
                });
            }
        };
        scheduler.submit_batch(
            &self.control,
            self.class,
            options,
            operations,
            run,
            application_failed,
        )
    }

    /// Creates an open retained group in this lane. Seal it before waiting for
    /// completion; dropping an observer never cancels its accepted members.
    pub fn group(&self) -> Result<Group, SpawnError> {
        self.control
            .owned_scheduler_for(self.class)?
            .group(self.class)
    }

    /// Admits a job without waiting for capacity or executing it on this caller.
    /// Any returned value, including `Result::Err`, counts as success.
    /// Rejection returns the uninvoked operation. Dropping observers never cancels work.
    pub fn try_spawn<F, T>(&self, options: SpawnOptions, operation: F) -> SpawnResult<T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.submit_owned(
            OwnedRoute::new(options),
            operation,
            call_once::<F, T>,
            never_fail::<T>,
        )
    }

    /// Admits a job without waiting for capacity or executing it on this caller.
    /// Membership is admitted to an open group from this lane.
    /// Any returned value, including `Result::Err`, counts as success.
    /// Rejection returns the uninvoked operation. Dropping observers never cancels work.
    pub fn try_spawn_in<F, T>(
        &self,
        group: &Group,
        options: SpawnOptions,
        operation: F,
    ) -> SpawnResult<T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.submit_owned(
            OwnedRoute {
                group: Some(group),
                ..OwnedRoute::new(options)
            },
            operation,
            call_once::<F, T>,
            never_fail::<T>,
        )
    }

    /// Admits a job without waiting for capacity or executing it on this caller.
    /// A returned `Err` marks application failure for success-only dependents;
    /// the typed error stays in the result. Prerequisite policy is independent.
    /// Rejection returns the uninvoked operation. Dropping observers never cancels work.
    pub fn try_spawn_fallible<F, T, E>(
        &self,
        options: SpawnOptions,
        operation: F,
    ) -> SpawnResult<Result<T, E>, F>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        self.submit_owned(
            OwnedRoute::new(options),
            operation,
            call_once::<F, Result<T, E>>,
            result_is_err::<T, E>,
        )
    }

    /// Admits a job without waiting for capacity or executing it on this caller.
    /// Membership is admitted to an open group from this lane.
    /// A returned `Err` marks application failure for success-only dependents;
    /// the typed error stays in the result. Prerequisite policy is independent.
    /// Rejection returns the uninvoked operation. Dropping observers never cancels work.
    pub fn try_spawn_fallible_in<F, T, E>(
        &self,
        group: &Group,
        options: SpawnOptions,
        operation: F,
    ) -> SpawnResult<Result<T, E>, F>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        self.submit_owned(
            OwnedRoute {
                group: Some(group),
                ..OwnedRoute::new(options)
            },
            operation,
            call_once::<F, Result<T, E>>,
            result_is_err::<T, E>,
        )
    }

    /// May execute caller-eligible work synchronously on runnable saturation.
    /// Record/edge limits still apply. Handoff pressure alone never runs it inline.
    /// Any returned value, including `Result::Err`, counts as success.
    /// Rejection returns the uninvoked operation. Dropping observers never cancels work.
    pub fn submit_or_run<F, T>(&self, options: SpawnOptions, operation: F) -> SpawnResult<T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.submit_owned(
            OwnedRoute {
                allow_inline: true,
                ..OwnedRoute::new(options)
            },
            operation,
            call_once::<F, T>,
            never_fail::<T>,
        )
    }

    /// May execute caller-eligible work synchronously on runnable saturation.
    /// Record/edge limits still apply. Handoff pressure alone never runs it inline.
    /// Membership is admitted to an open group from this lane.
    /// Any returned value, including `Result::Err`, counts as success.
    /// Rejection returns the uninvoked operation. Dropping observers never cancels work.
    pub fn submit_or_run_in<F, T>(
        &self,
        group: &Group,
        options: SpawnOptions,
        operation: F,
    ) -> SpawnResult<T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.submit_owned(
            OwnedRoute {
                group: Some(group),
                allow_inline: true,
                ..OwnedRoute::new(options)
            },
            operation,
            call_once::<F, T>,
            never_fail::<T>,
        )
    }

    /// May execute caller-eligible work synchronously on runnable saturation.
    /// Record/edge limits still apply. Handoff pressure alone never runs it inline.
    /// A returned `Err` marks application failure for success-only dependents;
    /// the typed error stays in the result. Prerequisite policy is independent.
    /// Rejection returns the uninvoked operation. Dropping observers never cancels work.
    pub fn submit_or_run_fallible<F, T, E>(
        &self,
        options: SpawnOptions,
        operation: F,
    ) -> SpawnResult<Result<T, E>, F>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        self.submit_owned(
            OwnedRoute {
                allow_inline: true,
                ..OwnedRoute::new(options)
            },
            operation,
            call_once::<F, Result<T, E>>,
            result_is_err::<T, E>,
        )
    }

    /// May execute caller-eligible work synchronously on runnable saturation.
    /// Record/edge limits still apply. Handoff pressure alone never runs it inline.
    /// Membership is admitted to an open group from this lane.
    /// A returned `Err` marks application failure for success-only dependents;
    /// the typed error stays in the result. Prerequisite policy is independent.
    /// Rejection returns the uninvoked operation. Dropping observers never cancels work.
    pub fn submit_or_run_fallible_in<F, T, E>(
        &self,
        group: &Group,
        options: SpawnOptions,
        operation: F,
    ) -> SpawnResult<Result<T, E>, F>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        self.submit_owned(
            OwnedRoute {
                group: Some(group),
                allow_inline: true,
                ..OwnedRoute::new(options)
            },
            operation,
            call_once::<F, Result<T, E>>,
            result_is_err::<T, E>,
        )
    }

    /// Admits a successor without occupying a worker while prerequisites wait.
    /// Outcome-aware work must inspect its captured prerequisite outcomes.
    /// Any returned value, including `Result::Err`, counts as success.
    /// Rejection returns the uninvoked operation. Dropping observers never cancels work.
    pub fn try_spawn_after<F, T>(
        &self,
        options: SpawnOptions,
        prerequisites: &[Completion],
        policy: DependencyPolicy,
        operation: F,
    ) -> SpawnResult<T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.submit_owned(
            OwnedRoute {
                prerequisites,
                policy,
                ..OwnedRoute::new(options)
            },
            operation,
            call_once::<F, T>,
            never_fail::<T>,
        )
    }

    /// Admits a successor without occupying a worker while prerequisites wait.
    /// Outcome-aware work must inspect its captured prerequisite outcomes.
    /// Membership is admitted to an open group from this lane.
    /// Any returned value, including `Result::Err`, counts as success.
    /// Rejection returns the uninvoked operation. Dropping observers never cancels work.
    pub fn try_spawn_after_in<F, T>(
        &self,
        group: &Group,
        options: SpawnOptions,
        prerequisites: &[Completion],
        policy: DependencyPolicy,
        operation: F,
    ) -> SpawnResult<T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.submit_owned(
            OwnedRoute {
                group: Some(group),
                prerequisites,
                policy,
                ..OwnedRoute::new(options)
            },
            operation,
            call_once::<F, T>,
            never_fail::<T>,
        )
    }

    /// Admits a successor without occupying a worker while prerequisites wait.
    /// Outcome-aware work must inspect its captured prerequisite outcomes.
    /// A returned `Err` marks application failure for success-only dependents;
    /// the typed error stays in the result. Prerequisite policy is independent.
    /// Rejection returns the uninvoked operation. Dropping observers never cancels work.
    pub fn try_spawn_after_fallible<F, T, E>(
        &self,
        options: SpawnOptions,
        prerequisites: &[Completion],
        policy: DependencyPolicy,
        operation: F,
    ) -> SpawnResult<Result<T, E>, F>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        self.submit_owned(
            OwnedRoute {
                prerequisites,
                policy,
                ..OwnedRoute::new(options)
            },
            operation,
            call_once::<F, Result<T, E>>,
            result_is_err::<T, E>,
        )
    }

    /// Admits a successor without occupying a worker while prerequisites wait.
    /// Outcome-aware work must inspect its captured prerequisite outcomes.
    /// Membership is admitted to an open group from this lane.
    /// A returned `Err` marks application failure for success-only dependents;
    /// the typed error stays in the result. Prerequisite policy is independent.
    /// Rejection returns the uninvoked operation. Dropping observers never cancels work.
    pub fn try_spawn_after_fallible_in<F, T, E>(
        &self,
        group: &Group,
        options: SpawnOptions,
        prerequisites: &[Completion],
        policy: DependencyPolicy,
        operation: F,
    ) -> SpawnResult<Result<T, E>, F>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        self.submit_owned(
            OwnedRoute {
                group: Some(group),
                prerequisites,
                policy,
                ..OwnedRoute::new(options)
            },
            operation,
            call_once::<F, Result<T, E>>,
            result_is_err::<T, E>,
        )
    }

    fn submit_owned<P, T>(
        &self,
        route: OwnedRoute<'_>,
        payload: P,
        run: fn(P) -> T,
        application_failed: fn(&T) -> bool,
    ) -> SpawnResult<T, P>
    where
        P: Send + 'static,
        T: Send + 'static,
    {
        let scheduler = match self.control.owned_scheduler_for(self.class) {
            Ok(scheduler) => scheduler,
            Err(reason) => {
                return Err(SpawnRejected {
                    reason,
                    operation: payload,
                    options: route.options,
                });
            }
        };
        scheduler.submit_payload(
            SubmitRequest {
                control: &self.control,
                class: self.class,
                group: route.group,
                options: route.options,
                prerequisites: route.prerequisites,
                policy: route.policy,
                allow_inline: route.allow_inline,
            },
            payload,
            run,
            application_failed,
        )
    }

    pub(crate) fn try_spawn_with_payload<P, T>(
        &self,
        options: SpawnOptions,
        prerequisites: &[Completion],
        policy: DependencyPolicy,
        payload: P,
        run: fn(P) -> T,
        application_failed: fn(&T) -> bool,
    ) -> SpawnResult<T, P>
    where
        P: Send + 'static,
        T: Send + 'static,
    {
        self.submit_owned(
            OwnedRoute {
                prerequisites,
                policy,
                ..OwnedRoute::new(options)
            },
            payload,
            run,
            application_failed,
        )
    }
}
