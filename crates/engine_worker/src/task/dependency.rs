//! Typed continuations over already admitted prerequisites.

use super::{Outcome, ProducerControl, Shared, Task};
use crate::execution::Lane;
use crate::scheduler::{
    DependencyPolicy, SpawnError, SpawnOptions, SpawnRejected, never_fail, result_is_err,
};

/// Rejected continuation inputs. Admission consumes neither the input handle
/// nor the operation, and the operation has not run.
pub struct ThenRejected<I, F> {
    pub reason: SpawnError,
    pub input: I,
    pub operation: F,
    pub options: SpawnOptions,
}

impl<I, F> std::fmt::Debug for ThenRejected<I, F> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ThenRejected")
            .field("reason", &self.reason)
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

fn run_unique_success<T, U, F>((mut task, operation): (Task<T>, F)) -> U
where
    T: Send + 'static,
    F: FnOnce(T) -> U,
{
    match task.try_take() {
        Some(Outcome::Success(value)) => operation(value),
        _ => unreachable!("success-only continuation invoked without a successful input"),
    }
}

fn run_unique_outcome<T, U, F>((mut task, operation): (Task<T>, F)) -> U
where
    T: Send + 'static,
    F: FnOnce(Outcome<T>) -> U,
{
    let outcome = task
        .try_take()
        .expect("outcome-aware continuation invoked before input completion");
    operation(outcome)
}

fn run_shared<T, U, F>((shared, operation): (Shared<T>, F)) -> U
where
    T: Send + Sync + 'static,
    F: FnOnce(Shared<T>) -> U,
{
    operation(shared)
}

/// An accepted continuation and its cancellation authority, or unchanged inputs.
pub type ThenResult<T, I, F> = Result<(Task<T>, ProducerControl), ThenRejected<I, F>>;

impl<I, F> From<SpawnRejected<(I, F)>> for ThenRejected<I, F> {
    fn from(rejected: SpawnRejected<(I, F)>) -> Self {
        let (input, operation) = rejected.operation;
        Self {
            reason: rejected.reason,
            input,
            operation,
            options: rejected.options,
        }
    }
}

impl<T: Send + 'static> Task<T> {
    /// Admits a successor transferring the unique input on acceptance.
    /// Suppresses the operation unless the prerequisite succeeded.
    /// Any returned value, including `Result::Err`, counts as success.
    /// Rejection returns the input and uninvoked operation. A previously taken
    /// result is rejected with [`SpawnError::Consumed`].
    pub fn try_then<U, F>(
        self,
        lane: &Lane,
        options: SpawnOptions,
        operation: F,
    ) -> ThenResult<U, Self, F>
    where
        U: Send + 'static,
        F: FnOnce(T) -> U + Send + 'static,
    {
        self.then(
            lane,
            options,
            DependencyPolicy::SuccessOnly,
            operation,
            run_unique_success::<T, U, F>,
            never_fail::<U>,
        )
    }

    /// Admits a successor transferring the unique input on acceptance.
    /// Suppresses the operation unless the prerequisite succeeded.
    /// A returned `Err` marks application failure while retaining its typed error.
    /// Rejection returns the input and uninvoked operation. A previously taken
    /// result is rejected with [`SpawnError::Consumed`].
    pub fn try_then_fallible<U, E, F>(
        self,
        lane: &Lane,
        options: SpawnOptions,
        operation: F,
    ) -> ThenResult<Result<U, E>, Self, F>
    where
        U: Send + 'static,
        E: Send + 'static,
        F: FnOnce(T) -> Result<U, E> + Send + 'static,
    {
        self.then(
            lane,
            options,
            DependencyPolicy::SuccessOnly,
            operation,
            run_unique_success::<T, Result<U, E>, F>,
            result_is_err::<U, E>,
        )
    }

    /// Admits a successor transferring the unique input on acceptance.
    /// Runs even if the prerequisite failed; inspect its outcome inside the operation.
    /// Any returned value, including `Result::Err`, counts as success.
    /// Rejection returns the input and uninvoked operation. A previously taken
    /// result is rejected with [`SpawnError::Consumed`].
    pub fn try_then_outcome<U, F>(
        self,
        lane: &Lane,
        options: SpawnOptions,
        operation: F,
    ) -> ThenResult<U, Self, F>
    where
        U: Send + 'static,
        F: FnOnce(Outcome<T>) -> U + Send + 'static,
    {
        self.then(
            lane,
            options,
            DependencyPolicy::OutcomeAware,
            operation,
            run_unique_outcome::<T, U, F>,
            never_fail::<U>,
        )
    }

    /// Admits a successor transferring the unique input on acceptance.
    /// Runs even if the prerequisite failed; inspect its outcome inside the operation.
    /// A returned `Err` marks application failure while retaining its typed error.
    /// Rejection returns the input and uninvoked operation. A previously taken
    /// result is rejected with [`SpawnError::Consumed`].
    pub fn try_then_outcome_fallible<U, E, F>(
        self,
        lane: &Lane,
        options: SpawnOptions,
        operation: F,
    ) -> ThenResult<Result<U, E>, Self, F>
    where
        U: Send + 'static,
        E: Send + 'static,
        F: FnOnce(Outcome<T>) -> Result<U, E> + Send + 'static,
    {
        self.then(
            lane,
            options,
            DependencyPolicy::OutcomeAware,
            operation,
            run_unique_outcome::<T, Result<U, E>, F>,
            result_is_err::<U, E>,
        )
    }

    fn then<U, F>(
        self,
        lane: &Lane,
        options: SpawnOptions,
        policy: DependencyPolicy,
        operation: F,
        run: fn((Self, F)) -> U,
        application_failed: fn(&U) -> bool,
    ) -> ThenResult<U, Self, F>
    where
        U: Send + 'static,
        F: Send + 'static,
    {
        if !self.continuation_input_available() {
            return Err(ThenRejected {
                reason: SpawnError::Consumed,
                input: self,
                operation,
                options,
            });
        }
        let prerequisite = self.completion();
        lane.try_spawn_with_payload(
            options,
            &[prerequisite],
            policy,
            (self, operation),
            run,
            application_failed,
        )
        .map_err(ThenRejected::from)
    }
}
impl<T: Send + Sync + 'static> Shared<T> {
    /// Admits a successor with shared immutable input.
    /// Suppresses the operation unless the prerequisite succeeded.
    /// Any returned value, including `Result::Err`, counts as success.
    /// Rejection returns the input and uninvoked operation. A previously taken
    /// result is rejected with [`SpawnError::Consumed`].
    pub fn try_then<U, F>(
        self,
        lane: &Lane,
        options: SpawnOptions,
        operation: F,
    ) -> ThenResult<U, Self, F>
    where
        U: Send + 'static,
        F: FnOnce(Shared<T>) -> U + Send + 'static,
    {
        self.then(
            lane,
            options,
            DependencyPolicy::SuccessOnly,
            operation,
            run_shared::<T, U, F>,
            never_fail::<U>,
        )
    }

    /// Admits a successor with shared immutable input.
    /// Suppresses the operation unless the prerequisite succeeded.
    /// A returned `Err` marks application failure while retaining its typed error.
    /// Rejection returns the input and uninvoked operation. A previously taken
    /// result is rejected with [`SpawnError::Consumed`].
    pub fn try_then_fallible<U, E, F>(
        self,
        lane: &Lane,
        options: SpawnOptions,
        operation: F,
    ) -> ThenResult<Result<U, E>, Self, F>
    where
        U: Send + 'static,
        E: Send + 'static,
        F: FnOnce(Shared<T>) -> Result<U, E> + Send + 'static,
    {
        self.then(
            lane,
            options,
            DependencyPolicy::SuccessOnly,
            operation,
            run_shared::<T, Result<U, E>, F>,
            result_is_err::<U, E>,
        )
    }

    /// Admits a successor with shared immutable input.
    /// Runs even if the prerequisite failed; inspect its outcome inside the operation.
    /// Any returned value, including `Result::Err`, counts as success.
    /// Rejection returns the input and uninvoked operation. A previously taken
    /// result is rejected with [`SpawnError::Consumed`].
    pub fn try_then_outcome<U, F>(
        self,
        lane: &Lane,
        options: SpawnOptions,
        operation: F,
    ) -> ThenResult<U, Self, F>
    where
        U: Send + 'static,
        F: FnOnce(Shared<T>) -> U + Send + 'static,
    {
        self.then(
            lane,
            options,
            DependencyPolicy::OutcomeAware,
            operation,
            run_shared::<T, U, F>,
            never_fail::<U>,
        )
    }

    /// Admits a successor with shared immutable input.
    /// Runs even if the prerequisite failed; inspect its outcome inside the operation.
    /// A returned `Err` marks application failure while retaining its typed error.
    /// Rejection returns the input and uninvoked operation. A previously taken
    /// result is rejected with [`SpawnError::Consumed`].
    pub fn try_then_outcome_fallible<U, E, F>(
        self,
        lane: &Lane,
        options: SpawnOptions,
        operation: F,
    ) -> ThenResult<Result<U, E>, Self, F>
    where
        U: Send + 'static,
        E: Send + 'static,
        F: FnOnce(Shared<T>) -> Result<U, E> + Send + 'static,
    {
        self.then(
            lane,
            options,
            DependencyPolicy::OutcomeAware,
            operation,
            run_shared::<T, Result<U, E>, F>,
            result_is_err::<U, E>,
        )
    }

    fn then<U, F>(
        self,
        lane: &Lane,
        options: SpawnOptions,
        policy: DependencyPolicy,
        operation: F,
        run: fn((Self, F)) -> U,
        application_failed: fn(&U) -> bool,
    ) -> ThenResult<U, Self, F>
    where
        U: Send + 'static,
        F: Send + 'static,
    {
        if !self.continuation_input_available() {
            return Err(ThenRejected {
                reason: SpawnError::Consumed,
                input: self,
                operation,
                options,
            });
        }
        let prerequisite = self.completion();
        lane.try_spawn_with_payload(
            options,
            &[prerequisite],
            policy,
            (self, operation),
            run,
            application_failed,
        )
        .map_err(ThenRejected::from)
    }
}
