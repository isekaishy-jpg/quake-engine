//! Atomic admission of CPU work with a reserved owner publication.

use std::fmt;

use super::{Group, Lane};
use crate::owner::DeliveryTicket;
use crate::scheduler::{
    DependencyPolicy, SpawnError, SpawnOptions, SubmitRequest, call_once, never_fail, result_is_err,
};
use crate::task::{Completion, ProducerControl, Task};

/// CPU routing for a reserved owner delivery. Dependencies gate execution;
/// every terminal CPU outcome makes the delivery ready for owner inspection.
#[derive(Clone, Copy)]
pub struct DeliveryOptions<'a> {
    pub spawn: SpawnOptions,
    pub group: Option<&'a Group>,
    pub prerequisites: &'a [Completion],
    pub policy: DependencyPolicy,
}

impl Default for DeliveryOptions<'_> {
    fn default() -> Self {
        Self::new(SpawnOptions::default())
    }
}

impl DeliveryOptions<'_> {
    pub fn new(spawn: SpawnOptions) -> Self {
        Self {
            spawn,
            group: None,
            prerequisites: &[],
            policy: DependencyPolicy::SuccessOnly,
        }
    }
}

/// Rejection preserves the uninvoked operation and its uncommitted ticket.
pub struct DeliverySpawnRejected<'a, F> {
    pub reason: SpawnError,
    pub operation: F,
    pub ticket: DeliveryTicket,
    pub options: DeliveryOptions<'a>,
}

impl<F> fmt::Debug for DeliverySpawnRejected<'_, F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DeliverySpawnRejected")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

pub type DeliverySpawnResult<'a, T, F> =
    Result<(Task<T>, ProducerControl), DeliverySpawnRejected<'a, F>>;

impl Lane {
    /// Commits the reserved owner delivery before exposing CPU work. Rejection
    /// returns both inputs. Closing the owner after acceptance can suppress
    /// publication independently of CPU completion. Never invokes the owner callback.
    pub fn try_spawn_delivering<'a, F, T>(
        &self,
        options: DeliveryOptions<'a>,
        ticket: DeliveryTicket,
        operation: F,
    ) -> DeliverySpawnResult<'a, T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.submit_delivering(options, ticket, operation, never_fail::<T>, false)
    }

    /// As `try_spawn_delivering`, classifying a returned `Err` as application
    /// failure for dependent CPU work. Owner notification still occurs.
    pub fn try_spawn_fallible_delivering<'a, F, T, E>(
        &self,
        options: DeliveryOptions<'a>,
        ticket: DeliveryTicket,
        operation: F,
    ) -> DeliverySpawnResult<'a, Result<T, E>, F>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        self.submit_delivering(options, ticket, operation, result_is_err::<T, E>, false)
    }

    /// Allows explicitly caller-eligible work to run here when runnable capacity
    /// is full. The delivery is committed first and publication remains deferred.
    pub fn submit_or_run_delivering<'a, F, T>(
        &self,
        options: DeliveryOptions<'a>,
        ticket: DeliveryTicket,
        operation: F,
    ) -> DeliverySpawnResult<'a, T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        self.submit_delivering(options, ticket, operation, never_fail::<T>, true)
    }

    /// Fallible counterpart of `submit_or_run_delivering`.
    pub fn submit_or_run_fallible_delivering<'a, F, T, E>(
        &self,
        options: DeliveryOptions<'a>,
        ticket: DeliveryTicket,
        operation: F,
    ) -> DeliverySpawnResult<'a, Result<T, E>, F>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        self.submit_delivering(options, ticket, operation, result_is_err::<T, E>, true)
    }

    fn submit_delivering<'a, F, T>(
        &self,
        options: DeliveryOptions<'a>,
        ticket: DeliveryTicket,
        operation: F,
        application_failed: fn(&T) -> bool,
        allow_inline: bool,
    ) -> DeliverySpawnResult<'a, T, F>
    where
        F: FnOnce() -> T + Send + 'static,
        T: Send + 'static,
    {
        let scheduler = match self.control.owned_scheduler_for(self.class) {
            Ok(scheduler) => scheduler,
            Err(reason) => {
                return Err(DeliverySpawnRejected {
                    reason,
                    operation,
                    ticket,
                    options,
                });
            }
        };
        let mut delivery = Some(ticket);
        scheduler
            .submit_payload_delivering(
                SubmitRequest {
                    control: &self.control,
                    class: self.class,
                    group: options.group,
                    options: options.spawn,
                    prerequisites: options.prerequisites,
                    policy: options.policy,
                    allow_inline,
                },
                operation,
                call_once::<F, T>,
                application_failed,
                &mut delivery,
            )
            .map_err(|rejected| DeliverySpawnRejected {
                reason: rejected.reason,
                operation: rejected.operation,
                ticket: delivery.expect("rejected admission retains its delivery ticket"),
                options,
            })
    }
}
