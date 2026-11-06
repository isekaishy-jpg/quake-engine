//! Logical results supplied by an external provider.

use std::sync::{Arc, Mutex, Weak};

use crate::owner::DeliveryTicket;
use crate::scheduler::reservation::ByteLease;
use crate::scheduler::{
    Cost, DemandSnapshot, DiscoveryPermit, OwnedScheduler, Priority, Reservation, SpawnError,
    WorkSet,
};
use crate::task::{CompletionSink, Outcome, ProducerControl, Retained, Task, TaskStatus};

/// Optional admission, lifetime, publication, and demand policy for one
/// provider-supplied result. The provider is started only after admission.
#[derive(Default)]
pub struct ExternalOptions<'a> {
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
    /// Called outside scheduler locks with versioned aggregate demand.
    /// A panic is contained and that notification is discarded.
    /// Calls may overlap or arrive after logical settlement; the adapter must
    /// ignore older versions and validate its own provider lifetime before use.
    pub provider_demand: Option<Arc<dyn Fn(DemandSnapshot) + Send + Sync>>,
}

/// Accepted producer, reusable result and independent cancellation authority.
pub type ExternalResult<'a, T> =
    Result<(Producer<T>, Task<Retained<T>>, ProducerControl), ExternalRejected<'a>>;

/// Rejection returns all uncommitted options, including the delivery ticket.
pub struct ExternalRejected<'a> {
    /// Why it wasn't taken.
    pub reason: SpawnError,
    /// The options, handed back.
    pub options: ExternalOptions<'a>,
}

impl std::fmt::Debug for ExternalRejected<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalRejected")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

type PendingExternal<T> = (CompletionSink<Retained<T>>, Option<ByteLease>);

pub(crate) struct ExternalCore<T> {
    pending: Mutex<Option<PendingExternal<T>>>,
}

impl<T> ExternalCore<T> {
    pub(crate) fn new(sink: CompletionSink<Retained<T>>, bytes: Option<ByteLease>) -> Self {
        Self {
            pending: Mutex::new(Some((sink, bytes))),
        }
    }

    pub(crate) fn publish(&self, outcome: Outcome<T>, application_failed: bool) {
        let (sink, bytes) = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
            .expect("external result claimed exactly once");
        let outcome = match outcome {
            Outcome::Success(value) => Outcome::Success(Retained::new(value, bytes)),
            Outcome::Cancelled => Outcome::Cancelled,
            Outcome::PrerequisiteFailed => Outcome::PrerequisiteFailed,
            Outcome::Panicked => Outcome::Panicked,
            Outcome::Abandoned => Outcome::Abandoned,
        };
        sink.finish(outcome, application_failed);
    }

    pub(crate) fn publish_retained(&self, value: Retained<T>, application_failed: bool) {
        let (sink, unused_bytes) = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
            .expect("external result claimed exactly once");
        // A provider-owned output charge already follows the value. The
        // producer's optional retained charge is unused on this route.
        drop(unused_bytes);
        sink.finish(Outcome::Success(value), application_failed);
    }

    pub(crate) fn publish_status(&self, status: TaskStatus) {
        let outcome = match status {
            TaskStatus::Cancelled => Outcome::Cancelled,
            TaskStatus::Abandoned => Outcome::Abandoned,
            _ => unreachable!("external control settles cancellation or abandonment"),
        };
        self.publish(outcome, false);
    }
}

/// The single logical result authority for a provider. `complete` may be called
/// from any thread. Dropping an unfinished producer publishes abandonment. This
/// handle does not own or end a provider's separate physical access.
pub struct Producer<T: Send + 'static> {
    pub(crate) scheduler: Weak<OwnedScheduler>,
    pub(crate) id: u64,
    pub(crate) core: Arc<ExternalCore<T>>,
}

impl<T: Send + 'static> Producer<T> {
    /// Publishes a result if cancellation or abandonment has not already won.
    /// On a lost race, returns the untouched value to its caller. A successful
    /// result may precede physical retirement only when the value is already
    /// safe to observe independently; otherwise await provider acknowledgement
    /// before completing it.
    ///
    /// # Errors
    ///
    /// Hands the value back if it was cancelled or given up on first.
    pub fn complete(&self, value: T) -> Result<(), T> {
        self.complete_with(value, false)
    }

    /// Publishes a value whose declared bytes are already held by its
    /// physical-access lease. Configure `retained_bytes: 0` when using this
    /// route so the producer does not reserve a second output charge.
    ///
    /// # Errors
    ///
    /// Hands the value back if it was cancelled or given up on first.
    pub fn complete_retained(&self, value: Retained<T>) -> Result<(), Retained<T>> {
        self.complete_retained_with(value, false)
    }

    pub(crate) fn complete_with(&self, value: T, application_failed: bool) -> Result<(), T> {
        let Some(scheduler) = self.scheduler.upgrade() else {
            return Err(value);
        };
        if !scheduler.claim_external(self.id, false) {
            return Err(value);
        }
        scheduler.finish_external(self.id, || {
            self.core
                .publish(Outcome::Success(value), application_failed)
        });
        Ok(())
    }

    fn complete_retained_with(
        &self,
        value: Retained<T>,
        application_failed: bool,
    ) -> Result<(), Retained<T>> {
        let Some(scheduler) = self.scheduler.upgrade() else {
            return Err(value);
        };
        if !scheduler.claim_external(self.id, false) {
            return Err(value);
        }
        scheduler.finish_external(self.id, || {
            self.core.publish_retained(value, application_failed)
        });
        Ok(())
    }

    /// Requests logical cancellation. The provider must still retire any
    /// physical access through its independent access lease.
    pub fn cancel(&self) {
        if let Some(scheduler) = self.scheduler.upgrade() {
            scheduler.settle_external(self.id, TaskStatus::Cancelled);
        }
    }

    /// Settles an unfulfilled result as abandoned.
    pub fn abandon(&self) {
        if let Some(scheduler) = self.scheduler.upgrade() {
            scheduler.settle_external(self.id, TaskStatus::Abandoned);
        }
    }
}

impl<T: Send + 'static, E: Send + 'static> Producer<Result<T, E>> {
    /// Preserves the typed `Err` payload and marks dependency status as an
    /// application failure.
    ///
    /// # Errors
    ///
    /// Hands the value back if it was cancelled or given up on first.
    pub fn complete_fallible(&self, value: Result<T, E>) -> Result<(), Result<T, E>> {
        let failed = value.is_err();
        self.complete_with(value, failed)
    }

    /// Publishes an already charged application result while preserving a
    /// typed `Err` and its dependency failure status.
    ///
    /// # Errors
    ///
    /// Hands the value back if it was cancelled or given up on first.
    pub fn complete_fallible_retained(
        &self,
        value: Retained<Result<T, E>>,
    ) -> Result<(), Retained<Result<T, E>>> {
        let failed = value.is_err();
        self.complete_retained_with(value, failed)
    }
}

impl<T: Send + 'static> Drop for Producer<T> {
    fn drop(&mut self) {
        self.abandon();
    }
}
