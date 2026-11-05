//! Set-owned root and descendant submission through runtime-associated lanes.

use super::{Group, Lane};
use crate::scheduler::work_set::WorkSetLease;
use crate::scheduler::{
    DependencyPolicy, DiscoveryError, DiscoveryPermit, SpawnError, SpawnOptions, SpawnRejected,
    SpawnResult, SubmitRequest, WorkSet, call_once, never_fail, result_is_err,
};
use crate::task::Completion;

/// Owned submission choices for a work set root or discovery descendant.
/// Group membership and prerequisites commit with the job's admission.
#[derive(Clone, Copy)]
pub struct WorkOptions<'a> {
    pub spawn: SpawnOptions,
    pub group: Option<&'a Group>,
    pub prerequisites: &'a [Completion],
    pub dependency_policy: DependencyPolicy,
    pub allow_inline: bool,
}

impl Default for WorkOptions<'_> {
    fn default() -> Self {
        Self {
            spawn: SpawnOptions::default(),
            group: None,
            prerequisites: &[],
            dependency_policy: DependencyPolicy::SuccessOnly,
            allow_inline: false,
        }
    }
}

enum Source<'a> {
    Root(&'a WorkSet),
    Child(&'a DiscoveryPermit),
}

fn spawn<P, T>(
    source: Source<'_>,
    lane: &Lane,
    work: WorkOptions<'_>,
    operation: P,
    run: fn(P) -> T,
    application_failed: fn(&T) -> bool,
) -> SpawnResult<T, P>
where
    P: Send + 'static,
    T: Send + 'static,
{
    let options = work.spawn;
    let reject = |reason, operation| SpawnRejected {
        reason,
        operation,
        options,
    };
    let scheduler = match lane.control.owned_scheduler_for(lane.class) {
        Ok(scheduler) => scheduler,
        Err(reason) => return Err(reject(reason, operation)),
    };
    let identity = lane.control.identity();
    let lease_result = match source {
        Source::Root(set) => set.try_root_lease(identity),
        Source::Child(permit) => permit.try_child_lease(identity),
    };
    let lease: WorkSetLease = match lease_result {
        Ok(lease) => lease,
        Err(reason) => return Err(reject(map_discovery_error(reason), operation)),
    };
    scheduler.submit_payload_in_set(
        SubmitRequest {
            control: &lane.control,
            class: lane.class,
            group: work.group,
            options,
            prerequisites: work.prerequisites,
            policy: work.dependency_policy,
            allow_inline: work.allow_inline,
        },
        operation,
        run,
        application_failed,
        lease,
    )
}

fn map_discovery_error(error: DiscoveryError) -> SpawnError {
    match error {
        DiscoveryError::Full => SpawnError::Full,
        DiscoveryError::Closed | DiscoveryError::Cancelled => SpawnError::Closed,
    }
}

impl WorkSet {
    /// Admits a root job while the set is open. The accepted job keeps the set
    /// active until its capture, dependencies and cancellation settle.
    pub fn try_spawn<F, T>(
        &self,
        lane: &Lane,
        options: SpawnOptions,
        operation: F,
    ) -> SpawnResult<T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.try_spawn_with(
            lane,
            WorkOptions {
                spawn: options,
                ..WorkOptions::default()
            },
            operation,
        )
    }

    /// On runnable saturation, caller-eligible work may execute synchronously
    /// through the same set accounting and scheduler claim path.
    pub fn submit_or_run<F, T>(
        &self,
        lane: &Lane,
        options: SpawnOptions,
        operation: F,
    ) -> SpawnResult<T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.try_spawn_with(
            lane,
            WorkOptions {
                spawn: options,
                allow_inline: true,
                ..WorkOptions::default()
            },
            operation,
        )
    }

    /// Admits a root with optional group, prerequisites and caller eligibility.
    pub fn try_spawn_with<F, T>(
        &self,
        lane: &Lane,
        work: WorkOptions<'_>,
        operation: F,
    ) -> SpawnResult<T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        spawn(
            Source::Root(self),
            lane,
            work,
            operation,
            call_once::<F, T>,
            never_fail::<T>,
        )
    }

    /// A typed `Err` stays in the result and marks success-only dependents failed.
    pub fn try_spawn_fallible_with<F, T, E>(
        &self,
        lane: &Lane,
        work: WorkOptions<'_>,
        operation: F,
    ) -> SpawnResult<Result<T, E>, F>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        spawn(
            Source::Root(self),
            lane,
            work,
            operation,
            call_once::<F, Result<T, E>>,
            result_is_err::<T, E>,
        )
    }
}

impl DiscoveryPermit {
    /// Admits a descendant while this permit is live, including after the set
    /// is sealed. A child is counted before this permit can be released.
    pub fn try_spawn<F, T>(
        &self,
        lane: &Lane,
        options: SpawnOptions,
        operation: F,
    ) -> SpawnResult<T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.try_spawn_with(
            lane,
            WorkOptions {
                spawn: options,
                ..WorkOptions::default()
            },
            operation,
        )
    }

    /// Admits a descendant with caller execution on runnable saturation when
    /// the job explicitly allows that context.
    pub fn submit_or_run<F, T>(
        &self,
        lane: &Lane,
        options: SpawnOptions,
        operation: F,
    ) -> SpawnResult<T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.try_spawn_with(
            lane,
            WorkOptions {
                spawn: options,
                allow_inline: true,
                ..WorkOptions::default()
            },
            operation,
        )
    }

    /// Admits a descendant with optional group and prerequisite edges.
    pub fn try_spawn_with<F, T>(
        &self,
        lane: &Lane,
        work: WorkOptions<'_>,
        operation: F,
    ) -> SpawnResult<T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        spawn(
            Source::Child(self),
            lane,
            work,
            operation,
            call_once::<F, T>,
            never_fail::<T>,
        )
    }

    /// Admits a fallible descendant; the typed `Err` remains observable.
    pub fn try_spawn_fallible_with<F, T, E>(
        &self,
        lane: &Lane,
        work: WorkOptions<'_>,
        operation: F,
    ) -> SpawnResult<Result<T, E>, F>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        spawn(
            Source::Child(self),
            lane,
            work,
            operation,
            call_once::<F, Result<T, E>>,
            result_is_err::<T, E>,
        )
    }
}
