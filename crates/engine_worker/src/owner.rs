//! Thread-bound owner state and transferable delivery routes.

mod delivery;
mod phase;
mod sender;

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::marker::PhantomData;
use std::num::NonZeroUsize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::notification::NotifySource;
use crate::progress::Wake;
use crate::runtime::{
    OwnerCallbackGuard, OwnerRegistration, OwnerRootAdmission, RuntimeControl, RuntimeState,
};
use crate::scheduler::Reservation;
use crate::scheduler::work_set::{DiscoveryPermit, WorkSet, WorkSetLease};
use crate::{Completion, ExecutionError, Outcome, Shared, TaskStatus};
pub(crate) use delivery::TicketCommitError;
pub use delivery::{CancelResult, Delivery, DeliveryControl, DeliveryStatus, DeliveryTicket};
use delivery::{ClaimResult, Notification, SlotReservation, Transport};
pub use phase::{Phase, PumpBudget, PumpMode, PumpReport};
use sender::Inbox;
pub use sender::OwnerSender;

/// An expected owner operation failure. `Panicked` means a local callback
/// unwound and this owner has faulted; its state is not rolled back.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnerError {
    Closed,
    Full,
    InvalidContext,
    WrongPhase,
    Faulted,
    Panicked,
    NotReady,
}

/// Rejected registration returns the uninvoked local callback to its caller.
pub struct OwnerRejected<F> {
    pub reason: OwnerError,
    pub callback: F,
}

impl<F> fmt::Debug for OwnerRejected<F> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnerRejected")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

/// Result of immediate ready access. A pending result preserves its callback.
pub enum ReadyAccess<R, F> {
    Ready(R),
    Rejected(OwnerRejected<F>),
    Panicked,
}

/// A locally stored callback with its retained prerequisite registration.
struct LocalDelivery<O> {
    phase: Phase,
    callback: Box<dyn FnOnce(&mut O)>,
    subscription: Option<crate::task::Subscription>,
    work_set: Option<WorkSetLease>,
}

/// Returned by a promised local delivery reservation. The ticket is Send and
/// can be attached to admitted CPU work; the callback remains owner-local.
pub struct PreparedDelivery {
    pub ticket: DeliveryTicket,
    pub delivery: Delivery,
    pub control: DeliveryControl,
}

/// A callback-safe teardown request. The current mutable state borrow ends
/// before the owner performs local capture cleanup.
#[derive(Clone)]
pub struct OwnerControl {
    close_requested: Arc<AtomicBool>,
    wake: Arc<Wake>,
    notification_source: Option<NotifySource>,
}

impl OwnerControl {
    /// Requests closure for the next owner pump or callback return. This does
    /// not run cleanup on the requesting thread. Sender admission closes when
    /// the owner observes the request; no unclaimed callback then publishes.
    pub fn request_close(&self) {
        if !self.close_requested.swap(true, Ordering::AcqRel) {
            self.wake.notify();
            if let Some(source) = &self.notification_source {
                source.publish();
            }
        }
    }
}

/// A host-thread owner. `O` and local callbacks can be non-Send; the explicit
/// `Rc` marker prevents moving this value to another thread even when `O: Send`.
pub struct Owner<O> {
    state: O,
    control: Arc<RuntimeControl>,
    transport: Arc<Transport>,
    inbox: Arc<Inbox<O>>,
    registration: Option<OwnerRegistration>,
    phase: Option<Phase>,
    callbacks: HashMap<u64, LocalDelivery<O>>,
    pending: VecDeque<Notification>,
    closed: bool,
    faulted: bool,
    active: bool,
    close_requested: Arc<AtomicBool>,
    _thread_bound: PhantomData<Rc<()>>,
}

impl<O> Owner<O> {
    pub(crate) fn new(
        control: Arc<RuntimeControl>,
        state: O,
        capacity: NonZeroUsize,
    ) -> Result<Self, OwnerError> {
        let pool = control
            .owned_scheduler()
            .ok()
            .and_then(|scheduler| scheduler.capacity.clone());
        let transport = Transport::new_with_capacity(capacity, control.identity(), pool);
        transport.install_wake(control.wake());
        let notification_source = control.notification_domain().map(NotifySource::new);
        if let Some(source) = &notification_source {
            transport.install_notification_source(source.clone());
        }
        let for_close = Arc::clone(&transport);
        let for_progress = Arc::clone(&transport);
        let registration = control
            .register_owner(
                Arc::new(move || for_close.close()),
                Arc::new(move || for_progress.progress()),
                notification_source,
            )
            .map_err(|error| match error {
                ExecutionError::Closed => OwnerError::Closed,
                _ => OwnerError::InvalidContext,
            })?;
        let inbox = Inbox::new(Arc::clone(&transport), Arc::clone(&control));
        Ok(Self {
            state,
            control,
            transport,
            inbox,
            registration: Some(registration),
            phase: None,
            callbacks: HashMap::new(),
            pending: VecDeque::new(),
            closed: false,
            faulted: false,
            active: false,
            close_requested: Arc::new(AtomicBool::new(false)),
            _thread_bound: PhantomData,
        })
    }

    /// The host explicitly establishes the current application phase. It may
    /// then use immediate access or pump that phase's delivery route.
    pub fn set_phase(&mut self, phase: Phase) -> Result<(), OwnerError> {
        if self.active {
            return Err(OwnerError::InvalidContext);
        }
        self.phase = Some(phase);
        Ok(())
    }

    pub fn phase(&self) -> Option<Phase> {
        self.phase
    }

    pub fn control(&self) -> OwnerControl {
        OwnerControl {
            close_requested: Arc::clone(&self.close_requested),
            wake: self.control.wake(),
            notification_source: self.transport.notification_source(),
        }
    }

    pub(crate) fn notification_source(&self) -> Option<NotifySource> {
        self.transport.notification_source()
    }

    /// A transferable phase-addressed sender. Its callbacks must be Send;
    /// they still execute and are destroyed on this owner thread.
    pub fn sender(&self) -> OwnerSender<O> {
        self.inbox.sender()
    }

    /// State inspection is explicit host recovery after a caught callback
    /// panic. Calling this does not clear the fault automatically.
    pub fn state(&self) -> &O {
        &self.state
    }

    pub fn state_mut(&mut self) -> &mut O {
        &mut self.state
    }

    pub fn recover(&mut self) -> Result<(), OwnerError> {
        if self.closed || self.transport.is_closed() {
            return Err(OwnerError::Closed);
        }
        if !self.callbacks.is_empty() || !self.inbox.is_empty() || !self.transport.is_empty() {
            return Err(OwnerError::Faulted);
        }
        self.faulted = false;
        self.inbox.set_faulted(false);
        Ok(())
    }

    fn accept(&self, accounted_descendant: bool) -> Result<Option<OwnerRootAdmission>, OwnerError> {
        if self.closed || self.transport.is_closed() || self.close_requested.load(Ordering::Acquire)
        {
            Err(OwnerError::Closed)
        } else if self.faulted {
            Err(OwnerError::Faulted)
        } else if self.active || crate::notification::invocation_active() {
            Err(OwnerError::InvalidContext)
        } else if accounted_descendant {
            match self.control.phase() {
                RuntimeState::Running | RuntimeState::Closing => Ok(None),
                RuntimeState::Stopped | RuntimeState::Abandoned => Err(OwnerError::Closed),
            }
        } else {
            self.control
                .admit_owner_root()
                .map(Some)
                .map_err(|_| OwnerError::Closed)
        }
    }

    fn reserve<F>(&mut self, phase: Phase, callback: F) -> Result<SlotReservation, OwnerRejected<F>>
    where
        F: FnOnce(&mut O) + 'static,
    {
        self.reserve_with_lease(phase, callback, None)
    }

    fn reserve_with_lease<F>(
        &mut self,
        phase: Phase,
        callback: F,
        work_set: Option<WorkSetLease>,
    ) -> Result<SlotReservation, OwnerRejected<F>>
    where
        F: FnOnce(&mut O) + 'static,
    {
        self.reserve_with_capacity(phase, callback, work_set, None)
    }

    fn reserve_with_capacity<F>(
        &mut self,
        phase: Phase,
        callback: F,
        work_set: Option<WorkSetLease>,
        capacity: Option<&Reservation>,
    ) -> Result<SlotReservation, OwnerRejected<F>>
    where
        F: FnOnce(&mut O) + 'static,
    {
        let _admission = match self.accept(work_set.is_some()) {
            Ok(admission) => admission,
            Err(reason) => return Err(OwnerRejected { reason, callback }),
        };
        let reservation = if let Some(capacity) = capacity {
            match self.transport.reserve_reserved(capacity) {
                Ok(reservation) => reservation,
                Err(reason) => return Err(OwnerRejected { reason, callback }),
            }
        } else {
            let Some(reservation) = self.transport.reserve() else {
                return Err(OwnerRejected {
                    reason: if self.transport.is_closed() {
                        OwnerError::Closed
                    } else {
                        OwnerError::Full
                    },
                    callback,
                });
            };
            reservation
        };
        self.callbacks.insert(
            reservation.id(),
            LocalDelivery {
                phase,
                callback: Box::new(callback),
                subscription: None,
                work_set,
            },
        );
        Ok(reservation)
    }

    /// Reserves bounded notification capacity before an application admits
    /// producing work. Dropping an unused ticket suppresses and locally cleans
    /// the callback at the next owner pump or close.
    pub fn prepare_delivery<F>(
        &mut self,
        phase: Phase,
        callback: F,
    ) -> Result<PreparedDelivery, OwnerRejected<F>>
    where
        F: FnOnce(&mut O) + 'static,
    {
        self.prepare_with_capacity(None, None, phase, callback)
    }

    /// Reserves a promised delivery from a pipeline's delivery credits.
    /// The callback stays owner-local and its credit lasts through settlement.
    pub fn prepare_delivery_reserved<F>(
        &mut self,
        reservation: &Reservation,
        phase: Phase,
        callback: F,
    ) -> Result<PreparedDelivery, OwnerRejected<F>>
    where
        F: FnOnce(&mut O) + 'static,
    {
        self.prepare_with_capacity(None, Some(reservation), phase, callback)
    }

    /// Reserves a delivery owned by a consumer work set. Cancellation of that
    /// set suppresses this unclaimed callback without cancelling its producer.
    pub fn prepare_delivery_in<F>(
        &mut self,
        set: &WorkSet,
        phase: Phase,
        callback: F,
    ) -> Result<PreparedDelivery, OwnerRejected<F>>
    where
        F: FnOnce(&mut O) + 'static,
    {
        let lease = match set.try_consumer_lease(self.transport.runtime_identity()) {
            Ok(lease) => lease,
            Err(_) => {
                return Err(OwnerRejected {
                    reason: OwnerError::Closed,
                    callback,
                });
            }
        };
        self.prepare_with_capacity(Some(lease), None, phase, callback)
    }

    /// Reserves a consumer-owned promised delivery from a pipeline's credits.
    pub fn prepare_delivery_reserved_in<F>(
        &mut self,
        set: &WorkSet,
        reservation: &Reservation,
        phase: Phase,
        callback: F,
    ) -> Result<PreparedDelivery, OwnerRejected<F>>
    where
        F: FnOnce(&mut O) + 'static,
    {
        let lease = match set.try_consumer_lease(self.transport.runtime_identity()) {
            Ok(lease) => lease,
            Err(_) => {
                return Err(OwnerRejected {
                    reason: OwnerError::Closed,
                    callback,
                });
            }
        };
        self.prepare_with_capacity(Some(lease), Some(reservation), phase, callback)
    }

    /// Reserves a promised descendant delivery using an existing discoverer.
    pub fn prepare_delivery_from<F>(
        &mut self,
        permit: &DiscoveryPermit,
        phase: Phase,
        callback: F,
    ) -> Result<PreparedDelivery, OwnerRejected<F>>
    where
        F: FnOnce(&mut O) + 'static,
    {
        let lease = match permit.try_child_lease(self.transport.runtime_identity()) {
            Ok(lease) => lease,
            Err(_) => {
                return Err(OwnerRejected {
                    reason: OwnerError::Closed,
                    callback,
                });
            }
        };
        self.prepare_with_capacity(Some(lease), None, phase, callback)
    }

    /// Reserves a descendant consumer delivery from a pipeline's credits.
    pub fn prepare_delivery_reserved_from<F>(
        &mut self,
        permit: &DiscoveryPermit,
        reservation: &Reservation,
        phase: Phase,
        callback: F,
    ) -> Result<PreparedDelivery, OwnerRejected<F>>
    where
        F: FnOnce(&mut O) + 'static,
    {
        let lease = match permit.try_child_lease(self.transport.runtime_identity()) {
            Ok(lease) => lease,
            Err(_) => {
                return Err(OwnerRejected {
                    reason: OwnerError::Closed,
                    callback,
                });
            }
        };
        self.prepare_with_capacity(Some(lease), Some(reservation), phase, callback)
    }

    fn prepare_with_capacity<F>(
        &mut self,
        lease: Option<WorkSetLease>,
        capacity: Option<&Reservation>,
        phase: Phase,
        callback: F,
    ) -> Result<PreparedDelivery, OwnerRejected<F>>
    where
        F: FnOnce(&mut O) + 'static,
    {
        let reservation = self.reserve_with_capacity(phase, callback, lease, capacity)?;
        let control = reservation.control();
        if let Some(lease) = self
            .callbacks
            .get(&reservation.id())
            .and_then(|entry| entry.work_set.as_ref())
        {
            let cancel = control.clone();
            lease.register_cancel(Box::new(move || {
                cancel.cancel();
            }));
        }
        Ok(PreparedDelivery {
            delivery: reservation.observer(),
            control,
            ticket: reservation.ticket(),
        })
    }

    /// Queues a local callback for this phase. It never invokes inline.
    pub fn try_post<F>(
        &mut self,
        phase: Phase,
        callback: F,
    ) -> Result<(Delivery, DeliveryControl), OwnerRejected<F>>
    where
        F: FnOnce(&mut O) + 'static,
    {
        let reservation = self.reserve(phase, callback)?;
        let observer = reservation.observer();
        let control = reservation.control();
        reservation.notifier().ready();
        Ok((observer, control))
    }

    /// Subscribes to a CPU completion while retaining the callback locally.
    /// Ready-at-registration follows the same deferred path as pending work.
    pub fn on_ready<F>(
        &mut self,
        completion: &Completion,
        phase: Phase,
        callback: F,
    ) -> Result<(Delivery, DeliveryControl), OwnerRejected<F>>
    where
        F: FnOnce(&mut O, TaskStatus) + 'static,
    {
        self.on_ready_with_lease(completion, phase, callback, None, None)
    }

    /// Attaches a consumer-owned delivery to an outcome. Cancelling this set only
    /// suppresses its callback; other consumers and the producer remain live.
    pub fn on_ready_in<F>(
        &mut self,
        set: &WorkSet,
        completion: &Completion,
        phase: Phase,
        callback: F,
    ) -> Result<(Delivery, DeliveryControl), OwnerRejected<F>>
    where
        F: FnOnce(&mut O, TaskStatus) + 'static,
    {
        let lease = match set.try_consumer_lease(self.transport.runtime_identity()) {
            Ok(lease) => lease,
            Err(_) => {
                return Err(OwnerRejected {
                    reason: OwnerError::Closed,
                    callback,
                });
            }
        };
        self.on_ready_with_lease(completion, phase, callback, Some(lease), None)
    }

    /// Attaches a descendant subscription using ordinary delivery capacity,
    /// including after the consumer set is sealed or runtime roots close.
    /// Ready outcomes still deliver through the owner pump. Set cancellation
    /// suppresses only this consumer's unclaimed callback; rejection returns
    /// the uninvoked callback without retaining a consumer lease.
    pub fn on_ready_from<F>(
        &mut self,
        permit: &DiscoveryPermit,
        completion: &Completion,
        phase: Phase,
        callback: F,
    ) -> Result<(Delivery, DeliveryControl), OwnerRejected<F>>
    where
        F: FnOnce(&mut O, TaskStatus) + 'static,
    {
        let lease = match permit.try_child_lease(self.transport.runtime_identity()) {
            Ok(lease) => lease,
            Err(_) => {
                return Err(OwnerRejected {
                    reason: OwnerError::Closed,
                    callback,
                });
            }
        };
        self.on_ready_with_lease(completion, phase, callback, Some(lease), None)
    }

    /// Subscribes to an existing outcome using reserved delivery capacity.
    /// Pending and already-ready outcomes both notify through the owner pump;
    /// no CPU job is created. Rejection returns the uninvoked callback and
    /// restores any provisionally checked-out reservation credits.
    pub fn on_ready_reserved<F>(
        &mut self,
        reservation: &Reservation,
        completion: &Completion,
        phase: Phase,
        callback: F,
    ) -> Result<(Delivery, DeliveryControl), OwnerRejected<F>>
    where
        F: FnOnce(&mut O, TaskStatus) + 'static,
    {
        self.on_ready_with_lease(completion, phase, callback, None, Some(reservation))
    }

    /// A reserved subscription owned by a consumer set, independent of the
    /// producer's set. Its credit and set count last through owner-local cleanup.
    pub fn on_ready_reserved_in<F>(
        &mut self,
        set: &WorkSet,
        reservation: &Reservation,
        completion: &Completion,
        phase: Phase,
        callback: F,
    ) -> Result<(Delivery, DeliveryControl), OwnerRejected<F>>
    where
        F: FnOnce(&mut O, TaskStatus) + 'static,
    {
        let lease = match set.try_consumer_lease(self.transport.runtime_identity()) {
            Ok(lease) => lease,
            Err(_) => {
                return Err(OwnerRejected {
                    reason: OwnerError::Closed,
                    callback,
                });
            }
        };
        self.on_ready_with_lease(completion, phase, callback, Some(lease), Some(reservation))
    }

    /// Attaches a reserved descendant subscription after the consumer set has
    /// sealed. Cancelling that set suppresses only this consumer's publication.
    pub fn on_ready_reserved_from<F>(
        &mut self,
        permit: &DiscoveryPermit,
        reservation: &Reservation,
        completion: &Completion,
        phase: Phase,
        callback: F,
    ) -> Result<(Delivery, DeliveryControl), OwnerRejected<F>>
    where
        F: FnOnce(&mut O, TaskStatus) + 'static,
    {
        let lease = match permit.try_child_lease(self.transport.runtime_identity()) {
            Ok(lease) => lease,
            Err(_) => {
                return Err(OwnerRejected {
                    reason: OwnerError::Closed,
                    callback,
                });
            }
        };
        self.on_ready_with_lease(completion, phase, callback, Some(lease), Some(reservation))
    }

    fn on_ready_with_lease<F>(
        &mut self,
        completion: &Completion,
        phase: Phase,
        callback: F,
        work_set: Option<WorkSetLease>,
        capacity: Option<&Reservation>,
    ) -> Result<(Delivery, DeliveryControl), OwnerRejected<F>>
    where
        F: FnOnce(&mut O, TaskStatus) + 'static,
    {
        let _admission = match self.accept(work_set.is_some()) {
            Ok(admission) => admission,
            Err(reason) => return Err(OwnerRejected { reason, callback }),
        };
        let reservation = if let Some(capacity) = capacity {
            match self.transport.reserve_reserved(capacity) {
                Ok(reservation) => reservation,
                Err(reason) => return Err(OwnerRejected { reason, callback }),
            }
        } else {
            let Some(reservation) = self.transport.reserve() else {
                return Err(OwnerRejected {
                    reason: if self.transport.is_closed() {
                        OwnerError::Closed
                    } else {
                        OwnerError::Full
                    },
                    callback,
                });
            };
            reservation
        };
        let observed = completion.clone();
        self.callbacks.insert(
            reservation.id(),
            LocalDelivery {
                phase,
                callback: Box::new(move |state| {
                    let status = observed
                        .status()
                        .expect("ready delivery has published completion status");
                    callback(state, status);
                }),
                subscription: None,
                work_set,
            },
        );
        let id = reservation.id();
        let observer = reservation.observer();
        let control = reservation.control();
        let ticket = reservation.ticket();
        let subscription = completion.subscribe_cancelable(Box::new(move |_| ticket.ready()));
        self.callbacks
            .get_mut(&id)
            .expect("reserved callback remains local")
            .subscription = Some(subscription);
        if let Some(lease) = self
            .callbacks
            .get(&id)
            .and_then(|entry| entry.work_set.as_ref())
        {
            let cancel = control.clone();
            lease.register_cancel(Box::new(move || {
                cancel.cancel();
            }));
        }
        Ok((observer, control))
    }

    /// Runs a ready shared outcome synchronously in the selected valid phase.
    /// A pending input returns the untouched callback. Caught panics fault all
    /// ordinary routes; the host may inspect and explicitly recover or close.
    pub fn with_ready<T, F, R>(
        &mut self,
        result: &Shared<T>,
        phase: Phase,
        callback: F,
    ) -> ReadyAccess<R, F>
    where
        T: Send + Sync + 'static,
        F: FnOnce(&mut O, Arc<Outcome<T>>) -> R,
    {
        let _admission = match self.accept(false) {
            Ok(admission) => admission,
            Err(reason) => {
                return ReadyAccess::Rejected(OwnerRejected { reason, callback });
            }
        };
        if self.phase != Some(phase) {
            return ReadyAccess::Rejected(OwnerRejected {
                reason: OwnerError::WrongPhase,
                callback,
            });
        }
        let Some(value) = result.try_result() else {
            return ReadyAccess::Rejected(OwnerRejected {
                reason: OwnerError::NotReady,
                callback,
            });
        };
        self.active = true;
        let outcome = {
            let _guard = OwnerCallbackGuard::enter();
            catch_unwind(AssertUnwindSafe(|| callback(&mut self.state, value)))
        };
        self.active = false;
        let access = match outcome {
            Ok(value) => ReadyAccess::Ready(value),
            Err(payload) => {
                let _guard = OwnerCallbackGuard::enter();
                crate::cleanup::discard_panic(payload);
                self.fault();
                ReadyAccess::Panicked
            }
        };
        self.finish_close_request();
        access
    }

    fn receive(&mut self) {
        while let Some(notification) = self.transport.try_recv() {
            if let Some((phase, callback)) = self.inbox.take(notification.id()) {
                self.callbacks.insert(
                    notification.id(),
                    LocalDelivery {
                        phase,
                        callback,
                        subscription: None,
                        work_set: None,
                    },
                );
            }
            self.pending.push_back(notification);
        }
    }

    fn next_for(&mut self, phase: Phase, all_phases: bool) -> Option<Notification> {
        if all_phases {
            return self.pending.pop_front();
        }
        let index = self.pending.iter().position(|notification| {
            self.callbacks
                .get(&notification.id())
                .is_some_and(|entry| entry.phase == phase)
        })?;
        self.pending.remove(index)
    }

    /// Runs or cleans eligible entries within the caller's count/time budget.
    /// No callback or local destructor executes under transport locks.
    pub fn pump(&mut self, phase: Phase, budget: PumpBudget) -> Result<PumpReport, OwnerError> {
        if self.active || crate::notification::invocation_active() {
            return Err(OwnerError::InvalidContext);
        }
        self.finish_close_request();
        if self.transport.is_closed() {
            self.closed = true;
        }
        if self.phase != Some(phase) && !self.closed && !self.faulted && !self.transport.is_closed()
        {
            return Err(OwnerError::WrongPhase);
        }
        self.receive();
        // Batch does not receive new notifications during this pump, and the
        // phase of each pending callback is immutable. Freezing this predicate
        // therefore preserves the entry identities without allocating an ID list.
        // Closure/fault may suppress those entries, but cannot substitute entries
        // from another phase into their remaining budget.
        let entry_all_phases = self.closed || self.faulted || self.transport.is_closed();
        let frontier = if budget.mode == PumpMode::Batch {
            self.pending
                .iter()
                .filter(|notification| {
                    if entry_all_phases {
                        return true;
                    }
                    self.callbacks
                        .get(&notification.id())
                        .is_some_and(|entry| entry.phase == phase)
                })
                .count()
        } else {
            usize::MAX
        };
        let started = Instant::now();
        let mut report = PumpReport::default();
        while report.processed() < budget.max_entries && report.processed() < frontier {
            if budget
                .max_duration
                .is_some_and(|duration| started.elapsed() >= duration)
            {
                break;
            }
            if budget.mode == PumpMode::Live {
                self.receive();
            }
            let all_phases = if budget.mode == PumpMode::Batch {
                entry_all_phases
            } else {
                self.closed || self.faulted || self.transport.is_closed()
            };
            let Some(notification) = self.next_for(phase, all_phases) else {
                break;
            };
            self.process(notification, &mut report);
        }
        self.unregister_if_drained();
        Ok(report)
    }

    fn process(&mut self, notification: Notification, report: &mut PumpReport) {
        let id = notification.id();
        let Some(mut entry) = self.callbacks.remove(&id) else {
            notification.claim();
            notification.settle(DeliveryStatus::Suppressed);
            report.suppressed += 1;
            return;
        };
        let claimed = notification.claim();
        // Detach unresolved prerequisite capture before callback/destructor
        // cleanup, outside transport synchronization.
        entry.subscription.take();
        let work_set = entry.work_set.take();
        // The synchronized claim is the cutoff, including runtime abandonment.
        // Rechecking transport closure here would retract an accepted claim.
        if claimed == ClaimResult::Suppress {
            let status = self.discard_entry(entry);
            notification.settle(status);
            drop(work_set);
            report.suppressed += 1;
            // Local destructors may request teardown just like callbacks.
            self.finish_close_request();
            return;
        }
        self.active = true;
        let result = {
            let _guard = OwnerCallbackGuard::enter();
            catch_unwind(AssertUnwindSafe(|| (entry.callback)(&mut self.state)))
        };
        self.active = false;
        match result {
            Ok(()) => {
                notification.settle(DeliveryStatus::Published);
                report.invoked += 1;
            }
            Err(payload) => {
                let _guard = OwnerCallbackGuard::enter();
                crate::cleanup::discard_panic(payload);
                notification.settle(DeliveryStatus::Panicked);
                report.suppressed += 1;
                self.fault();
            }
        }
        drop(work_set);
        self.finish_close_request();
    }

    fn finish_close_request(&mut self) {
        if self.close_requested.swap(false, Ordering::AcqRel) {
            self.closed = true;
            self.inbox.mark_closed();
        }
        self.unregister_if_drained();
    }

    fn unregister_if_drained(&mut self) {
        if self.closed
            && self.callbacks.is_empty()
            && self.inbox.is_empty()
            && self.transport.is_empty()
        {
            self.registration.take();
        }
    }

    fn fault(&mut self) {
        self.faulted = true;
        self.inbox.set_faulted(true);
        self.transport.suppress_all();
    }

    fn cleanup_all(&mut self) {
        self.receive();
        while let Some(notification) = self.pending.pop_front() {
            let id = notification.id();
            notification.claim();
            let (work_set, status) = if let Some(mut entry) = self.callbacks.remove(&id) {
                let work_set = entry.work_set.take();
                let status = self.discard_entry(entry);
                (work_set, status)
            } else {
                (None, DeliveryStatus::Suppressed)
            };
            notification.settle(status);
            drop(work_set);
        }
    }

    /// Contain capture-destruction panics so callers can settle notification and
    /// work-set accounting afterward and continue cleaning the remaining routes.
    fn discard_entry(&mut self, entry: LocalDelivery<O>) -> DeliveryStatus {
        let _guard = OwnerCallbackGuard::enter();
        match catch_unwind(AssertUnwindSafe(|| drop(entry))) {
            Ok(()) => DeliveryStatus::Suppressed,
            Err(payload) => {
                crate::cleanup::discard_panic(payload);
                self.fault();
                DeliveryStatus::Panicked
            }
        }
    }

    /// Rejects new registrations and suppresses unclaimed callbacks. Local
    /// cleanup finishes before runtime owner registration is released.
    /// Capture-destruction panics fault the owner and settle that delivery as
    /// Panicked; cleanup continues for the remaining entries.
    pub fn close(&mut self) {
        self.closed = true;
        let transferred = self.inbox.close_and_take();
        for (id, phase, callback) in transferred {
            self.callbacks.insert(
                id,
                LocalDelivery {
                    phase,
                    callback,
                    subscription: None,
                    work_set: None,
                },
            );
        }
        self.transport.close();
        self.cleanup_all();
        self.registration.take();
    }
}

impl<O> Drop for Owner<O> {
    fn drop(&mut self) {
        self.close();
    }
}
