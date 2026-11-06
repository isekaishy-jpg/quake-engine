//! Retained results and status-only observation.

use std::any::Any;
use std::collections::HashMap;
use std::ops::Deref;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, Weak};
use std::time::Duration;

use crate::execution::context;
use crate::notification::{NotifyDomain, NotifySource};
use crate::scheduler::storage::SignalRetirement;

/// A completed owned invocation. Application errors remain in a successful
/// returned `Result<T, E>`; they are never erased into a runtime failure.
#[derive(Debug, Eq, PartialEq)]
pub enum Outcome<T> {
    /// It finished with this.
    Success(T),
    /// It was cancelled.
    Cancelled,
    /// What came first failed.
    PrerequisiteFailed,
    /// It panicked.
    Panicked,
    /// It was given up on.
    Abandoned,
}

/// Status used by dependencies. `ApplicationFailed` means a fallible task
/// returned `Err`; its typed error remains in the result cell.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TaskStatus {
    /// It finished.
    Succeeded,
    /// It finished with a failure of its own.
    ApplicationFailed,
    /// It was cancelled.
    Cancelled,
    /// What came first failed.
    PrerequisiteFailed,
    /// It panicked.
    Panicked,
    /// It was given up on.
    Abandoned,
}

impl TaskStatus {
    /// Whether it finished.
    pub fn is_success(self) -> bool {
        self == Self::Succeeded
    }
}

/// Passive waiting is unavailable during CPU work or on a live owner thread.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WaitError {
    /// It can't be waited on from here.
    ExecutionContext,
}

type Subscriber = Box<dyn FnOnce(TaskStatus) + Send + 'static>;

struct SignalState {
    status: Option<TaskStatus>,
    subscribers: HashMap<u64, Subscriber>,
    next_subscriber: u64,
}

pub(crate) struct Signal {
    state: Mutex<SignalState>,
    changed: Condvar,
    producer: OnceLock<ProducerIdentity>,
    runtime_identity: OnceLock<u64>,
    notification_domain: OnceLock<Weak<NotifyDomain>>,
    notification: OnceLock<NotifySource>,
}

struct ProducerIdentity {
    runtime: u64,
    record: u64,
    scheduler: Weak<crate::scheduler::OwnedScheduler>,
}

impl Signal {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(SignalState {
                status: None,
                subscribers: HashMap::new(),
                next_subscriber: 1,
            }),
            changed: Condvar::new(),
            producer: OnceLock::new(),
            runtime_identity: OnceLock::new(),
            notification_domain: OnceLock::new(),
            notification: OnceLock::new(),
        }
    }

    pub(crate) fn reset(&mut self) {
        let state = self
            .state
            .get_mut()
            .unwrap_or_else(|error| error.into_inner());
        debug_assert!(state.subscribers.is_empty());
        state.status = None;
        state.next_subscriber = 1;
        self.producer.take();
        self.notification.take();
        self.notification_domain.take();
        self.runtime_identity.take();
    }

    pub(crate) fn is_reusable(&self) -> bool {
        let state = self.lock();
        state.status.is_some() && state.subscribers.is_empty()
    }

    fn lock(&self) -> MutexGuard<'_, SignalState> {
        self.state.lock().unwrap_or_else(|error| error.into_inner())
    }

    fn set_runtime_identity(&self, runtime: u64) {
        assert!(
            self.runtime_identity.set(runtime).is_ok(),
            "completion runtime installed once before publication"
        );
    }

    fn set_notification_source(&self, domain: &Arc<NotifyDomain>) {
        debug_assert!(self.runtime_identity.get().is_some());
        assert!(
            self.notification_domain.set(Arc::downgrade(domain)).is_ok(),
            "notification domain installed once before publication"
        );
    }

    fn notification_source(&self) -> Option<NotifySource> {
        let state = self.lock();
        let domain = self.notification_domain.get()?.upgrade()?;
        Some(
            self.notification
                .get_or_init(|| {
                    let source = NotifySource::new(&domain);
                    // This source is not visible to a binding until this lock
                    // is released. A concurrent publisher either sees it
                    // under this same lock or has already committed status.
                    if state.status.is_some() {
                        source.publish_terminal();
                    }
                    source
                })
                .clone(),
        )
    }

    fn publish(&self, status: TaskStatus) {
        self.publish_notifying(status, || {});
    }

    fn publish_notifying(&self, status: TaskStatus, notify: impl FnOnce()) {
        let (subscribers, source) = {
            let mut state = self.lock();
            debug_assert!(state.status.is_none(), "completion published twice");
            state.status = Some(status);
            self.changed.notify_all();
            (
                std::mem::take(&mut state.subscribers),
                self.notification.get().cloned(),
            )
        };
        // Wake the group's separate helping predicate after status is visible,
        // before arbitrary downstream activation or cleanup can block/unwind.
        #[cfg(feature = "diagnostics")]
        if let Some(source) = &source {
            crate::diagnostics::record(
                "completion.visible",
                source.runtime_id(),
                source.serial(),
                0,
            );
        }
        notify();
        if let Some(source) = source {
            source.publish_terminal();
        }
        // Activation never runs under the result or status lock.
        let mut first_panic = None;
        for (_, subscriber) in subscribers {
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| subscriber(status))) {
                if first_panic.is_none() {
                    first_panic = Some(payload);
                } else {
                    crate::cleanup::discard_panic(payload);
                }
            }
        }
        if let Some(payload) = first_panic {
            resume_unwind(payload);
        }
    }
}

/// A signal returns to its scheduler cache after its last owning observation
/// ends. Weak subscriptions pin the identity and prevent reuse.
pub(crate) struct SignalLease {
    signal: Option<Arc<Signal>>,
    retirement: Weak<SignalRetirement>,
}

impl SignalLease {
    pub(crate) fn unpooled(signal: Arc<Signal>) -> Self {
        Self {
            signal: Some(signal),
            retirement: Weak::new(),
        }
    }

    pub(crate) fn pooled(signal: Arc<Signal>, retirement: Weak<SignalRetirement>) -> Self {
        Self {
            signal: Some(signal),
            retirement,
        }
    }

    fn arc(&self) -> &Arc<Signal> {
        self.signal.as_ref().expect("signal lease is live")
    }

    fn get_mut(&mut self) -> Option<&mut Signal> {
        Arc::get_mut(self.signal.as_mut().expect("signal lease is live"))
    }
}

impl Clone for SignalLease {
    fn clone(&self) -> Self {
        Self {
            signal: Some(Arc::clone(self.arc())),
            retirement: self.retirement.clone(),
        }
    }
}

impl Deref for SignalLease {
    type Target = Signal;

    fn deref(&self) -> &Self::Target {
        self.arc()
    }
}

impl Drop for SignalLease {
    fn drop(&mut self) {
        let signal = self.signal.take().expect("signal lease drops once");
        if let Some(retirement) = self.retirement.upgrade() {
            retirement.retire_if_last(signal);
        }
    }
}

/// A status-only prerequisite. Task readiness means the invocation and its
/// captures have settled and its outcome is available, but successor edges may
/// still be activating. A group token instead includes its sealed members'
/// settlement bookkeeping. Neither token includes its own downstream work.
/// Cloning observes the same completion; dropping an observer does not request
/// cancellation.
#[derive(Clone)]
pub struct Completion {
    signal: SignalLease,
}

impl Completion {
    pub(crate) fn pending() -> Self {
        Self {
            signal: SignalLease::unpooled(Arc::new(Signal::new())),
        }
    }

    pub(crate) fn reset(&mut self) {
        if let Some(signal) = self.signal.get_mut() {
            signal.reset();
        } else {
            *self = Self::pending();
        }
    }

    pub(crate) fn publish_notifying(&self, status: TaskStatus, notify: impl FnOnce()) {
        self.signal.publish_notifying(status, notify);
    }

    pub(crate) fn set_runtime_identity(&self, runtime: u64) {
        self.signal.set_runtime_identity(runtime);
    }

    pub(crate) fn set_notification_source(&self, domain: &Arc<NotifyDomain>) {
        self.signal.set_notification_source(domain);
    }

    pub(crate) fn notification_source(&self) -> Option<NotifySource> {
        self.signal.notification_source()
    }

    pub(crate) fn notification_runtime_id(&self) -> Option<u64> {
        self.signal.runtime_identity.get().copied()
    }

    pub(crate) fn same_signal(&self, other: &Self) -> bool {
        Arc::ptr_eq(self.signal.arc(), other.signal.arc())
    }

    pub(crate) fn producer_identity(&self) -> Option<(u64, u64)> {
        self.signal
            .producer
            .get()
            .map(|producer| (producer.runtime, producer.record))
    }

    /// Retains independent consumer interest while this producer is pending.
    /// Dropping demand never cancels the producer or releases a retained result.
    ///
    /// # Errors
    ///
    /// Returns an error if demand is closed or full, or what it was for has gone.
    pub fn demand(
        &self,
        priority: crate::scheduler::Priority,
    ) -> Result<crate::scheduler::Demand, crate::scheduler::DemandError> {
        let producer = self
            .signal
            .producer
            .get()
            .ok_or(crate::scheduler::DemandError::Closed)?;
        let scheduler = producer
            .scheduler
            .upgrade()
            .ok_or(crate::scheduler::DemandError::Closed)?;
        scheduler.attach_demand(producer.record, priority)
    }

    /// Retains consumer interest in a separate work set until this producer
    /// completes, the set is cancelled, or the demand handle is dropped.
    /// Detaching this interest never cancels the shared producer.
    ///
    /// # Errors
    ///
    /// Returns an error if demand is closed or full, or what it was for has gone.
    pub fn demand_in(
        &self,
        set: &crate::scheduler::WorkSet,
        priority: crate::scheduler::Priority,
    ) -> Result<crate::scheduler::Demand, crate::scheduler::DemandError> {
        let producer = self
            .signal
            .producer
            .get()
            .ok_or(crate::scheduler::DemandError::Closed)?;
        let lease = set
            .try_consumer_lease(producer.runtime)
            .map_err(|_| crate::scheduler::DemandError::Closed)?;
        let demand = self.demand(priority)?;
        Ok(demand.bind_consumer(lease, self))
    }

    /// Retains descendant consumer interest through an existing discovery
    /// permit, including after the set is sealed or runtime roots close.
    /// Completion, consumer cancellation, or dropping the demand detaches this
    /// interest without cancelling the producer. A cancelled/foreign permit
    /// returns `Closed`; other demand errors match [`Self::demand`]. Failed
    /// attachment retains no consumer lease.
    ///
    /// # Errors
    ///
    /// Returns an error if demand is closed or full, or what it was for has gone.
    pub fn demand_from(
        &self,
        permit: &crate::scheduler::DiscoveryPermit,
        priority: crate::scheduler::Priority,
    ) -> Result<crate::scheduler::Demand, crate::scheduler::DemandError> {
        let producer = self
            .signal
            .producer
            .get()
            .ok_or(crate::scheduler::DemandError::Closed)?;
        let lease = permit
            .try_child_lease(producer.runtime)
            .map_err(|_| crate::scheduler::DemandError::Closed)?;
        let demand = self.demand(priority)?;
        Ok(demand.bind_consumer(lease, self))
    }

    /// How it finished, if it has.
    pub fn status(&self) -> Option<TaskStatus> {
        self.signal.lock().status
    }

    /// Waits for the represented outcome without executing jobs or servicing
    /// owner/provider callbacks. This can return while successor activation is
    /// still in progress. Group tokens include their members' settlement.
    ///
    /// # Errors
    ///
    /// Returns an error if it can't wait from here.
    pub fn wait(&self) -> Result<TaskStatus, WaitError> {
        if context::passive_wait_forbidden() {
            return Err(WaitError::ExecutionContext);
        }
        let mut state = self.signal.lock();
        loop {
            if let Some(status) = state.status {
                return Ok(status);
            }
            state = self
                .signal
                .changed
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
    }

    /// A timeout only ends observation; it never cancels the producer. A ready
    /// status has the same task-level boundary as [`Self::wait`].
    ///
    /// # Errors
    ///
    /// Returns an error if it can't wait from here.
    pub fn wait_timeout(&self, timeout: Duration) -> Result<Option<TaskStatus>, WaitError> {
        if context::passive_wait_forbidden() {
            return Err(WaitError::ExecutionContext);
        }
        let state = self.signal.lock();
        let (state, _) = self
            .signal
            .changed
            .wait_timeout_while(state, timeout, |state| state.status.is_none())
            .unwrap_or_else(|error| error.into_inner());
        Ok(state.status)
    }

    /// Registration and publication share one lock. An edge is delivered once
    /// whether attached before, during, or after completion. Publication makes
    /// the result visible before callbacks run so continuations can access it.
    pub(crate) fn subscribe_cancelable(&self, callback: Subscriber) -> Subscription {
        let ready = {
            let mut state = self.signal.lock();
            if let Some(status) = state.status {
                Err((status, callback))
            } else {
                let id = state.next_subscriber;
                state.next_subscriber = id.checked_add(1).expect("subscriber identity exhausted");
                state.subscribers.insert(id, callback);
                Ok(id)
            }
        };
        match ready {
            Ok(id) => Subscription {
                signal: Some(Arc::downgrade(self.signal.arc())),
                retirement: self.signal.retirement.clone(),
                id,
            },
            Err((status, callback)) => {
                callback(status);
                Subscription {
                    signal: None,
                    retirement: Weak::new(),
                    id: 0,
                }
            }
        }
    }
}

/// A charged dependency registration. Dropping it detaches an unresolved edge
/// promptly, including its captured scheduler reference.
pub(crate) struct Subscription {
    signal: Option<Weak<Signal>>,
    retirement: Weak<SignalRetirement>,
    id: u64,
}

impl Subscription {
    fn detach(&mut self) {
        let Some(signal) = self.signal.take().and_then(|signal| signal.upgrade()) else {
            return;
        };
        let signal = SignalLease {
            signal: Some(signal),
            retirement: self.retirement.clone(),
        };
        let callback = signal.lock().subscribers.remove(&self.id);
        drop(callback);
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.detach();
    }
}

struct ResultCell<T> {
    outcome: Mutex<ResultStorage<T>>,
    signal: SignalLease,
}

enum ResultStorage<T> {
    Unique(Option<Outcome<T>>),
    // The erased Arc keeps ResultCell<T> transferable for T: Send even when
    // T: !Sync. Only into_shared (which requires T: Sync) installs this mode
    // and its type-specific conversion function.
    Shared {
        outcome: Option<Box<dyn Any + Send + Sync>>,
        convert: fn(Outcome<T>) -> Box<dyn Any + Send + Sync>,
    },
}

impl<T> ResultCell<T> {
    fn continuation_input_available(&self) -> bool {
        let status = self.signal.lock();
        if status.status.is_none() {
            return true;
        }
        let storage = self
            .outcome
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        match &*storage {
            ResultStorage::Unique(outcome) => outcome.is_some(),
            ResultStorage::Shared { outcome, .. } => outcome.is_some(),
        }
    }
}

fn boxed_shared<T: Send + Sync + 'static>(outcome: Outcome<T>) -> Box<dyn Any + Send + Sync> {
    Box::new(Arc::new(outcome))
}

/// A borrowed completed outcome. The borrow never clones the payload and
/// cannot trigger execution.
pub struct OutcomeRef<'a, T> {
    guard: MutexGuard<'a, ResultStorage<T>>,
}

impl<T> Deref for OutcomeRef<'_, T> {
    type Target = Outcome<T>;

    fn deref(&self) -> &Self::Target {
        match &*self.guard {
            ResultStorage::Unique(Some(outcome)) => outcome,
            _ => unreachable!("unique outcome borrow exists only after completion"),
        }
    }
}

/// Unique ownership of an owned result. The value can be moved out once.
/// Readiness exposes the outcome after invocation capture cleanup; it does not
/// imply that every registered successor has activated.
pub struct Task<T: Send + 'static> {
    cell: Arc<ResultCell<T>>,
}

impl<T: Send + 'static> Task<T> {
    pub(crate) fn set_notification_source(&self, domain: &Arc<NotifyDomain>) {
        self.cell.signal.set_notification_source(domain);
    }

    pub(crate) fn set_producer(
        &self,
        runtime: u64,
        record: u64,
        scheduler: Weak<crate::scheduler::OwnedScheduler>,
    ) {
        self.cell.signal.set_runtime_identity(runtime);
        assert!(
            self.cell
                .signal
                .producer
                .set(ProducerIdentity {
                    runtime,
                    record,
                    scheduler
                })
                .is_ok(),
            "producer installed once before publication"
        );
    }
    pub(crate) fn continuation_input_available(&self) -> bool {
        self.cell.continuation_input_available()
    }

    pub(crate) fn pending_pair() -> (Self, CompletionSink<T>) {
        Self::pending_with_signal(SignalLease::unpooled(Arc::new(Signal::new())))
    }

    pub(crate) fn pending_with_signal(signal: SignalLease) -> (Self, CompletionSink<T>) {
        let cell = Arc::new(ResultCell {
            outcome: Mutex::new(ResultStorage::Unique(None)),
            signal,
        });
        (
            Self {
                cell: Arc::clone(&cell),
            },
            CompletionSink { cell: Some(cell) },
        )
    }

    /// Creates a ready value without admitting a CPU invocation.
    pub fn ready(value: T) -> Self {
        Self::ready_outcome(Outcome::Success(value))
    }

    /// Creates a ready fixed outcome without admitting a CPU invocation.
    pub fn ready_outcome(outcome: Outcome<T>) -> Self {
        let (task, sink) = Self::pending_pair();
        sink.finish(outcome, false);
        task
    }

    /// What says when it has finished.
    pub fn completion(&self) -> Completion {
        Completion {
            signal: self.cell.signal.clone(),
        }
    }

    /// How it finished, if it has.
    pub fn status(&self) -> Option<TaskStatus> {
        self.cell.signal.lock().status
    }

    /// Borrows the terminal outcome, if ready and still retained. A previous
    /// `try_take` leaves status ready but removes the value from this cell.
    pub fn try_result(&mut self) -> Option<OutcomeRef<'_, T>> {
        // The payload is stored before status publication. Holding the signal
        // lock until the payload lock is acquired keeps the two observations
        // consistent across the publication boundary.
        let status = self.cell.signal.lock();
        status.status?;
        let guard = self.cell.outcome.lock().unwrap_or_else(|e| e.into_inner());
        drop(status);
        if !matches!(&*guard, ResultStorage::Unique(Some(_))) {
            return None;
        }
        Some(OutcomeRef { guard })
    }

    /// Its outcome, if it has one and nothing has taken it.
    pub fn try_take(&mut self) -> Option<Outcome<T>> {
        let status = self.cell.signal.lock();
        status.status?;
        let mut outcome = self.cell.outcome.lock().unwrap_or_else(|e| e.into_inner());
        drop(status);
        match &mut *outcome {
            ResultStorage::Unique(outcome) => outcome.take(),
            ResultStorage::Shared { .. } => unreachable!("unique task was converted to shared"),
        }
    }

    /// Converts unique observation to clonable immutable observation. This
    /// allocates a shared outcome envelope now if ready, or at completion if
    /// pending. A result already taken remains absent. It never clones `T` or
    /// creates a CPU task.
    pub fn into_shared(self) -> Shared<T>
    where
        T: Send + Sync + 'static,
    {
        {
            let mut outcome = self.cell.outcome.lock().unwrap_or_else(|e| e.into_inner());
            let unique = match &mut *outcome {
                ResultStorage::Unique(unique) => unique.take(),
                ResultStorage::Shared { .. } => unreachable!("unique task converted twice"),
            };
            *outcome = ResultStorage::Shared {
                outcome: unique.map(boxed_shared::<T>),
                convert: boxed_shared::<T>,
            };
        }
        Shared { cell: self.cell }
    }
}

impl<T: Send + 'static, E: Send + 'static> Task<Result<T, E>> {
    /// Creates a ready typed application result. An `Err` keeps its payload
    /// while marking status as application failure for success-only edges.
    pub fn ready_fallible(value: Result<T, E>) -> Self {
        let failed = value.is_err();
        let (task, sink) = Self::pending_pair();
        sink.finish(Outcome::Success(value), failed);
        task
    }
}

/// Shared immutable observation of one result cell. A clone retains the cell;
/// it does not clone `T`. Readiness has the same task-level boundary as
/// [`Task`].
pub struct Shared<T: Send + Sync + 'static> {
    cell: Arc<ResultCell<T>>,
}

impl<T: Send + Sync + 'static> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self {
            cell: Arc::clone(&self.cell),
        }
    }
}

impl<T: Send + Sync + 'static> Shared<T> {
    pub(crate) fn continuation_input_available(&self) -> bool {
        self.cell.continuation_input_available()
    }

    /// A shared value that's already here.
    pub fn ready(value: T) -> Self {
        Task::ready(value).into_shared()
    }

    /// What says when it has finished.
    pub fn completion(&self) -> Completion {
        Completion {
            signal: self.cell.signal.clone(),
        }
    }

    /// How it finished, if it has.
    pub fn status(&self) -> Option<TaskStatus> {
        self.cell.signal.lock().status
    }

    /// Returns shared ownership of the same immutable outcome. The returned
    /// `Arc` supports simultaneous borrows and never clones `T`.
    ///
    /// # Panics
    ///
    /// Panics if shared storage has changed type, which is a bug in this crate.
    pub fn try_result(&self) -> Option<Arc<Outcome<T>>> {
        let status = self.cell.signal.lock();
        status.status?;
        let guard = self.cell.outcome.lock().unwrap_or_else(|e| e.into_inner());
        drop(status);
        match &*guard {
            ResultStorage::Shared { outcome, .. } => outcome.as_ref().map(|value| {
                Arc::clone(
                    value
                        .downcast_ref::<Arc<Outcome<T>>>()
                        .expect("shared storage retains its original outcome type"),
                )
            }),
            ResultStorage::Unique(_) => unreachable!("shared handle has unique storage"),
        }
    }
}

/// Scheduler-owned terminal publisher. Dropping an unfulfilled publisher
/// settles abandonment so an accepted observer cannot remain pending forever.
pub(crate) struct CompletionSink<T> {
    cell: Option<Arc<ResultCell<T>>>,
}

impl<T> CompletionSink<T> {
    pub(crate) fn finish(mut self, outcome: Outcome<T>, application_failed: bool) {
        let cell = self.cell.take().expect("completion sink is single-use");
        let status = match &outcome {
            Outcome::Success(_) if application_failed => TaskStatus::ApplicationFailed,
            Outcome::Success(_) => TaskStatus::Succeeded,
            Outcome::Cancelled => TaskStatus::Cancelled,
            Outcome::PrerequisiteFailed => TaskStatus::PrerequisiteFailed,
            Outcome::Panicked => TaskStatus::Panicked,
            Outcome::Abandoned => TaskStatus::Abandoned,
        };
        let mut storage = cell.outcome.lock().unwrap_or_else(|e| e.into_inner());
        match &mut *storage {
            ResultStorage::Unique(value) => *value = Some(outcome),
            ResultStorage::Shared {
                outcome: value,
                convert,
            } => *value = Some(convert(outcome)),
        }
        drop(storage);
        cell.signal.publish(status);
    }
}

impl<T> Drop for CompletionSink<T> {
    fn drop(&mut self) {
        if let Some(cell) = self.cell.take() {
            let mut storage = cell.outcome.lock().unwrap_or_else(|e| e.into_inner());
            match &mut *storage {
                ResultStorage::Unique(value) => *value = Some(Outcome::Abandoned),
                ResultStorage::Shared {
                    outcome: value,
                    convert,
                } => {
                    *value = Some(convert(Outcome::Abandoned));
                }
            }
            drop(storage);
            cell.signal.publish(TaskStatus::Abandoned);
        }
    }
}
