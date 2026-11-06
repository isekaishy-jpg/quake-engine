//! Coordination of accepted owned work.
//!
//! Class gates serialize acceptance, ready selection and claiming. Stable job
//! controls retain dependency and settlement state. Demand, global charges and
//! recyclers have separate synchronization domains.
//! User code, destructors, provider hooks, and backend calls run outside locks.

mod admission;
mod bulk;
mod demand;
mod prerequisite_range;
mod ready;
pub(crate) mod reservation;
mod scratch;
pub(crate) mod storage;
pub(crate) mod work_set;

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use crate::execution::ContextGuard;
use crate::execution::context;
use crate::execution::group::{Group, GroupInner};
use crate::external::producer::{
    ExternalCore, ExternalOptions, ExternalRejected, ExternalResult, Producer,
};
use crate::runtime::config::ExecutionClass;
use crate::runtime::{ExternalAdmission, OwnedAdmission, RuntimeControl};
use crate::task::{
    Completion, CompletionSink, Outcome, ProducerControl, Subscription, Task, TaskStatus,
};

pub use admission::{
    BatchSpawnOptions, BatchSpawnRejected, BatchSpawnResult, CallerEligibility, DependencyPolicy,
    OwnedConfigError, OwnedLimits, SpawnError, SpawnOptions, SpawnRejected, SpawnResult,
};
use demand::DemandState;
pub use demand::{Demand, DemandError, DemandSnapshot, Priority};
use ready::{PendingQueue, ReadyQueue};
pub use reservation::{
    ByteLease, CapacityUsage, Cost, LimitError, Limits, Reservation, ReservationError,
};
use reservation::{OwnedCapacity, ReservationPool};
use storage::{BufferPool, GroupPool, JobHandle, JobPool, SignalPool};
use work_set::WorkSetLease;
pub use work_set::{DiscoveryError, DiscoveryPermit, WorkSet, WorkSetProgress};

#[cfg(test)]
#[path = "../../tests/unit/demand.rs"]
mod demand_tests;

#[derive(Default)]
pub(crate) struct SubmitExtras {
    work_set: Option<WorkSetLease>,
    capacity: Option<OwnedCapacity>,
    priority: Option<Priority>,
    reserved: bool,
    batch_member: bool,
}

type Finish = Box<dyn FnOnce() + Send>;
type Envelope = Box<dyn JobEnvelope>;

trait JobEnvelope: Send {
    fn settle(self: Box<Self>, decision: Decision) -> Finish;
}

impl<F: FnOnce(Decision) -> Finish + Send> JobEnvelope for F {
    fn settle(self: Box<Self>, decision: Decision) -> Finish {
        self(decision)
    }
}

#[cfg(test)]
type AttachmentHook = Arc<dyn Fn(JobHandle) + Send + Sync>;

#[cfg(test)]
type CountHook = Arc<dyn Fn(usize) + Send + Sync>;

#[cfg(test)]
#[path = "../../tests/unit/scheduler_domains.rs"]
mod domain_tests;

enum Activation {
    Prerequisite(Arc<OwnedScheduler>, JobHandle, TaskStatus),
    PrerequisiteRange(
        Arc<OwnedScheduler>,
        prerequisite_range::PrerequisiteRange,
        TaskStatus,
    ),
    Finalize(Arc<OwnedScheduler>, JobHandle),
}

struct ActivationQueue {
    draining: bool,
    pending: VecDeque<Activation>,
}

thread_local! {
    // The control slot must remain accessible from later host TLS destructors.
    // Only the destructible cache retains an allocation while the drain is idle.
    static ACTIVATIONS: ManuallyDrop<RefCell<ActivationQueue>> = const { ManuallyDrop::new(RefCell::new(ActivationQueue {
        draining: false,
        pending: VecDeque::new(),
    })) };
    static ACTIVATION_STORAGE: RefCell<VecDeque<Activation>> = const { RefCell::new(VecDeque::new()) };
}

struct DrainGuard;

impl Drop for DrainGuard {
    fn drop(&mut self) {
        let mut buffer = ACTIVATIONS.with(|queue| {
            let mut queue = queue.borrow_mut();
            queue.draining = false;
            std::mem::take(&mut queue.pending)
        });
        debug_assert!(buffer.is_empty());
        // During thread teardown the cache may already be destroyed. In that
        // case the local buffer is freed here instead of being retained in TLS.
        let _ = ACTIVATION_STORAGE.try_with(|cache| {
            std::mem::swap(&mut *cache.borrow_mut(), &mut buffer);
        });
    }
}

fn enqueue_activation(activation: Activation) {
    let start = ACTIVATIONS.with(|queue| {
        let mut queue = queue.borrow_mut();
        if queue.draining {
            queue.pending.push_back(activation);
            false
        } else {
            queue.pending = ACTIVATION_STORAGE
                .try_with(|cache| std::mem::take(&mut *cache.borrow_mut()))
                .unwrap_or_default();
            queue.pending.push_back(activation);
            queue.draining = true;
            true
        }
    });
    if !start {
        return;
    }
    let guard = DrainGuard;
    let mut panic = None;
    loop {
        let next = ACTIVATIONS.with(|queue| queue.borrow_mut().pending.pop_front());
        let Some(next) = next else { break };
        let result = catch_unwind(AssertUnwindSafe(|| match next {
            Activation::Prerequisite(scheduler, id, status) => {
                scheduler.activate_prerequisite(&id, status)
            }
            Activation::PrerequisiteRange(scheduler, range, status) => {
                scheduler.activate_prerequisite_range(range.take_members(), status)
            }
            Activation::Finalize(scheduler, id) => scheduler.finalize_record(id),
        }));
        if let Err(payload) = result {
            if panic.is_none() {
                panic = Some(payload);
            } else {
                crate::cleanup::discard_panic(payload);
            }
        }
    }
    drop(guard);
    if let Some(payload) = panic {
        std::panic::resume_unwind(payload);
    }
}

#[derive(Clone, Copy)]
enum Decision {
    Run,
    Suppress(TaskStatus),
}

enum ClaimOutcome {
    Ran,
    Lost,
    Unavailable,
}

/// A backend wrapper occupies its handoff slot even if its logical job has
/// already settled. Discard during stop must release the slot without running
/// scheduler callbacks under the backend's queue lock.
struct Handoff {
    scheduler: Weak<OwnedScheduler>,
    class: ExecutionClass,
    completed: bool,
}

impl Handoff {
    fn run(mut self, job: JobHandle) {
        job.run();
        self.completed = true;
    }
}

impl Drop for Handoff {
    fn drop(&mut self) {
        if let Some(scheduler) = self.scheduler.upgrade() {
            scheduler.decrement_handoff(self.class);
            let abandoned = scheduler.abandoned.load(Ordering::Acquire);
            if self.completed && !abandoned {
                scheduler.dispatch_class(self.class);
            }
        }
    }
}

/// Owns physical slots selected but not yet transferred into backend wrappers.
/// Failure returns every unoffered slot once, independently of logical jobs.
struct ReservedHandoffs<'a> {
    scheduler: &'a OwnedScheduler,
    class: ExecutionClass,
    remaining: usize,
}

impl Drop for ReservedHandoffs<'_> {
    fn drop(&mut self) {
        if self.remaining != 0 {
            self.scheduler.class_lock(self.class).ready.handed_off -= self.remaining;
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Stage {
    Waiting,
    DeferredReady,
    Ready,
    Handed,
    Running,
    Finalizing,
}

struct Record {
    completion: Option<Completion>,
    group: Option<Arc<GroupInner>>,
    membership: Option<crate::execution::group::MembershipKey>,
    pending: usize,
    failed: bool,
    policy: DependencyPolicy,
    stage: Stage,
    eligibility: CallerEligibility,
    subscriptions: Vec<Subscription>,
    prerequisite_range: Option<prerequisite_range::RangeMember>,
    attaching: bool,
    deferred_finish: Option<Finish>,
    admission: OwnedAdmission,
    work_set: Option<WorkSetLease>,
    capacity: Option<OwnedCapacity>,
    resource: bool,
    runnable_reserved: bool,
    selection: Option<demand::DemandSelection>,
    charge: AccountingLease,
}

struct ExternalRecord {
    charge: AccountingLease,
    attaching: bool,
    deferred_status: Option<TaskStatus>,
    settling: bool,
    settle: Arc<dyn Fn(TaskStatus) + Send + Sync>,
    admission: ExternalAdmission,
    work_set: Option<WorkSetLease>,
    capacity: Option<Reservation>,
}

pub(crate) struct SubmitRequest<'a> {
    pub(crate) control: &'a Arc<RuntimeControl>,
    pub(crate) class: ExecutionClass,
    pub(crate) group: Option<&'a Group>,
    pub(crate) options: SpawnOptions,
    pub(crate) prerequisites: &'a [Completion],
    pub(crate) policy: DependencyPolicy,
    pub(crate) allow_inline: bool,
}

/// A charge is acquired before acceptance and retained through strong settlement.
/// CAS rollback keeps the record/edge limits global across all classes/providers.
struct Accounting {
    records: AtomicUsize,
    edges: AtomicUsize,
    limits: OwnedLimits,
}

struct AccountingLease {
    accounting: Arc<Accounting>,
    edges: usize,
}

impl Accounting {
    fn acquire(self: &Arc<Self>, edges: usize) -> Result<AccountingLease, SpawnError> {
        if edges > self.limits.edges {
            return Err(SpawnError::TooLarge);
        }
        self.records
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                (count < self.limits.records).then_some(count + 1)
            })
            .map_err(|_| SpawnError::Full)?;
        if edges != 0
            && self
                .edges
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                    count
                        .checked_add(edges)
                        .filter(|count| *count <= self.limits.edges)
                })
                .is_err()
        {
            self.records.fetch_sub(1, Ordering::AcqRel);
            return Err(SpawnError::Full);
        }
        Ok(AccountingLease {
            accounting: Arc::clone(self),
            edges,
        })
    }
}

impl Drop for AccountingLease {
    fn drop(&mut self) {
        if self.edges != 0 {
            self.accounting
                .edges
                .fetch_sub(self.edges, Ordering::AcqRel);
        }
        self.accounting.records.fetch_sub(1, Ordering::AcqRel);
    }
}

/// Ordinary operations enter exactly one class gate, then a record guard.
/// No record-only operation enters a class while retaining its record guard.
struct ClassState {
    records: HashMap<u64, JobHandle>,
    ready: ReadyQueue,
    deferred: PendingQueue,
    rebalance: bool,
    attaching_runnable: usize,
}

struct DemandDomain {
    graph: DemandState,
    targets: HashMap<u64, ExecutionClass>,
}

#[cfg(feature = "diagnostics")]
#[derive(Default)]
struct DomainMetrics {
    acquisitions: AtomicU64,
    wait_ns: AtomicU64,
    hold_ns: AtomicU64,
}

/// Notifications and diagnostic buffering follow the release of every guard.
struct DomainGuard<'a, T> {
    guard: Option<MutexGuard<'a, T>>,
    wake: &'a crate::progress::Wake,
    _notification: crate::notification::NotificationScope,
    #[cfg(feature = "diagnostics")]
    timing: Option<(std::time::Instant, std::time::Instant)>,
    #[cfg(feature = "diagnostics")]
    scheduler: u64,
    #[cfg(feature = "diagnostics")]
    runtime: u64,
    #[cfg(feature = "diagnostics")]
    domain: u64,
    #[cfg(feature = "diagnostics")]
    site: u32,
    #[cfg(feature = "diagnostics")]
    cycles: Option<u64>,
    #[cfg(feature = "diagnostics")]
    metrics: &'a DomainMetrics,
}

impl<T> std::ops::Deref for DomainGuard<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.guard.as_deref().expect("live domain guard")
    }
}
impl<T> std::ops::DerefMut for DomainGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.guard.as_deref_mut().expect("live domain guard")
    }
}
impl<T> Drop for DomainGuard<'_, T> {
    #[cfg_attr(
        feature = "diagnostics",
        expect(
            clippy::cast_possible_truncation,
            reason = "held to the most a u64 can be just before"
        )
    )]
    fn drop(&mut self) {
        #[cfg(feature = "diagnostics")]
        let cycles = self
            .cycles
            .and_then(|start| crate::diagnostics::cycles().map(|end| end.saturating_sub(start)));
        #[cfg(feature = "diagnostics")]
        let released = self.timing.map(|_| std::time::Instant::now());
        drop(self.guard.take());
        #[cfg(feature = "diagnostics")]
        if let (Some((started, acquired)), Some(released)) = (self.timing, released) {
            let waiting = acquired
                .duration_since(started)
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64;
            let holding = released
                .duration_since(acquired)
                .as_nanos()
                .min(u128::from(u64::MAX)) as u64;
            self.metrics.acquisitions.fetch_add(1, Ordering::Relaxed);
            self.metrics.wait_ns.fetch_add(waiting, Ordering::Relaxed);
            self.metrics.hold_ns.fetch_add(holding, Ordering::Relaxed);
            if waiting >= 50_000 {
                crate::diagnostics::record_at(
                    "scheduler.lock.slow",
                    self.runtime,
                    self.scheduler,
                    waiting,
                    acquired,
                );
                crate::diagnostics::record_at(
                    "scheduler.domain.lock.slow",
                    self.runtime,
                    self.domain,
                    waiting,
                    acquired,
                );
            }
            if holding >= 50_000 {
                crate::diagnostics::record_at(
                    "scheduler.hold.slow",
                    self.runtime,
                    self.scheduler,
                    holding,
                    released,
                );
                crate::diagnostics::record_at(
                    "scheduler.domain.hold.slow",
                    self.runtime,
                    self.domain,
                    holding,
                    released,
                );
                crate::diagnostics::record_at(
                    "scheduler.hold.site",
                    self.runtime,
                    self.scheduler,
                    u64::from(self.site),
                    released,
                );
                if let Some(cycles) = cycles {
                    crate::diagnostics::record_at(
                        "scheduler.hold.cycles",
                        self.runtime,
                        self.scheduler,
                        cycles,
                        released,
                    );
                }
            }
        }
        self.wake.notify();
    }
}

struct ProviderCallbacks<'a> {
    scheduler: &'a OwnedScheduler,
    count: usize,
}
impl Drop for ProviderCallbacks<'_> {
    fn drop(&mut self) {
        self.scheduler
            .provider_callbacks
            .fetch_sub(self.count, Ordering::AcqRel);
        self.scheduler.wake.notify();
    }
}

/// Stable controls and independently coordinated execution classes.
pub(crate) struct OwnedScheduler {
    control: Weak<RuntimeControl>,
    #[cfg(feature = "diagnostics")]
    trace_runtime: u64,
    #[cfg(feature = "diagnostics")]
    metrics: [DomainMetrics; 5],
    limits: OwnedLimits,
    classes: [Mutex<ClassState>; 3],
    external: Mutex<HashMap<u64, ExternalRecord>>,
    abandoned: AtomicBool,
    next_id: AtomicU64,
    next_group: AtomicU64,
    accounting: Arc<Accounting>,
    demand: Mutex<DemandDomain>,
    // Snapshot publication serializes only graph and bounded class mailboxes,
    // never queue/record gates. Each class applies only its own mailbox.
    demand_transfer: Mutex<()>,
    demand_pending: AtomicBool,
    demand_updates: [Mutex<HashMap<u64, demand::DemandSelection>>; 3],
    demand_updates_pending: [AtomicBool; 3],
    demand_enabled: bool,
    provider_callbacks: AtomicUsize,
    jobs: JobPool,
    signals: SignalPool,
    scratch: [scratch::ScratchPool; 3],
    subscriptions: Mutex<BufferPool<Subscription>>,
    groups: GroupPool,
    pub(crate) capacity: Option<ReservationPool>,
    wake: Arc<crate::progress::Wake>,
    #[cfg(test)]
    attachment_hook: Mutex<Option<AttachmentHook>>,
    #[cfg(test)]
    batch_portion_hook: Mutex<Option<CountHook>>,
    #[cfg(test)]
    handoff_hook: Mutex<Option<CountHook>>,
    #[cfg(test)]
    range_install_hook: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
    #[cfg(test)]
    range_delivery_hook: Mutex<Option<Arc<dyn Fn() + Send + Sync>>>,
}

impl OwnedScheduler {
    #[cfg(feature = "diagnostics")]
    fn trace(&self, event: &'static str, id: u64, related: u64) {
        crate::diagnostics::record(event, self.trace_runtime, id, related);
    }

    #[cfg(feature = "diagnostics")]
    fn trace_job(&self, job: &JobHandle) -> crate::diagnostics::JobGuard {
        crate::diagnostics::JobGuard::enter(crate::diagnostics::TraceJob {
            runtime: self.trace_runtime,
            id: job.id(),
            group: job.group_id(),
        })
    }

    #[track_caller]
    #[cfg_attr(
        feature = "diagnostics",
        expect(
            clippy::cast_possible_truncation,
            reason = "domains are a short fixed list"
        )
    )]
    fn domain_lock<'a, T>(&'a self, mutex: &'a Mutex<T>, _domain: u64) -> DomainGuard<'a, T> {
        let notification = self.wake.notification_scope();
        #[cfg(feature = "diagnostics")]
        let started = crate::diagnostics::clock();
        let guard = mutex.lock().unwrap_or_else(|error| error.into_inner());
        DomainGuard {
            guard: Some(guard),
            wake: &self.wake,
            _notification: notification,
            #[cfg(feature = "diagnostics")]
            timing: started.map(|start| (start, std::time::Instant::now())),
            #[cfg(feature = "diagnostics")]
            scheduler: std::ptr::from_ref(self) as usize as u64,
            #[cfg(feature = "diagnostics")]
            runtime: self.trace_runtime,
            #[cfg(feature = "diagnostics")]
            domain: _domain,
            #[cfg(feature = "diagnostics")]
            site: std::panic::Location::caller().line(),
            #[cfg(feature = "diagnostics")]
            cycles: started.and_then(|_| crate::diagnostics::cycles()),
            #[cfg(feature = "diagnostics")]
            metrics: &self.metrics[_domain as usize],
        }
    }

    #[track_caller]
    fn class_lock(&self, class: ExecutionClass) -> DomainGuard<'_, ClassState> {
        self.domain_lock(&self.classes[class.index()], class.index() as u64)
    }

    fn allocate_id(counter: &AtomicU64) -> u64 {
        counter
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .expect("scheduler identity exhausted")
    }

    pub(crate) fn new(
        control: Weak<RuntimeControl>,
        limits: OwnedLimits,
        capacity: Option<ReservationPool>,
        priorities: Vec<Priority>,
        demand_leases: usize,
    ) -> Arc<Self> {
        let wake = control.upgrade().expect("building live runtime").wake();
        if let Some(capacity) = &capacity {
            capacity.set_wake(Arc::clone(&wake));
        }
        Arc::new(Self {
            #[cfg(feature = "diagnostics")]
            trace_runtime: control.upgrade().expect("building live runtime").identity(),
            #[cfg(feature = "diagnostics")]
            metrics: std::array::from_fn(|_| DomainMetrics::default()),
            control,
            limits,
            capacity,
            wake,
            classes: std::array::from_fn(|_| {
                Mutex::new(ClassState {
                    records: HashMap::new(),
                    ready: ReadyQueue::with_priorities(&priorities),
                    deferred: PendingQueue::default(),
                    rebalance: false,
                    attaching_runnable: 0,
                })
            }),
            external: Mutex::new(HashMap::new()),
            abandoned: AtomicBool::new(false),
            next_id: AtomicU64::new(1),
            next_group: AtomicU64::new(1),
            accounting: Arc::new(Accounting {
                records: AtomicUsize::new(0),
                edges: AtomicUsize::new(0),
                limits,
            }),
            demand_enabled: !priorities.is_empty(),
            demand: Mutex::new(DemandDomain {
                graph: DemandState::new(priorities, demand_leases),
                targets: HashMap::new(),
            }),
            demand_transfer: Mutex::new(()),
            demand_pending: AtomicBool::new(false),
            demand_updates: std::array::from_fn(|_| Mutex::new(HashMap::new())),
            demand_updates_pending: std::array::from_fn(|_| AtomicBool::new(false)),
            provider_callbacks: AtomicUsize::new(0),
            jobs: JobPool::new(limits.records),
            signals: SignalPool::new(limits.records),
            scratch: std::array::from_fn(|_| scratch::ScratchPool::new()),
            subscriptions: Mutex::new(BufferPool::new(limits.edges)),
            groups: GroupPool::new(limits.records),
            #[cfg(test)]
            attachment_hook: Mutex::new(None),
            #[cfg(test)]
            batch_portion_hook: Mutex::new(None),
            #[cfg(test)]
            handoff_hook: Mutex::new(None),
            #[cfg(test)]
            range_install_hook: Mutex::new(None),
            #[cfg(test)]
            range_delivery_hook: Mutex::new(None),
        })
    }

    pub(crate) fn retire_scratch(&self) {
        for pool in &self.scratch {
            pool.retire();
        }
    }

    pub(crate) fn quiescent(&self) -> bool {
        self.accounting.records.load(Ordering::Acquire) == 0
            && self.classes.iter().all(|class| {
                class
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .ready
                    .handed_off
                    == 0
            })
            && !self.demand_pending.load(Ordering::Acquire)
            && self
                .demand_updates_pending
                .iter()
                .all(|pending| !pending.load(Ordering::Acquire))
            && self.provider_callbacks.load(Ordering::Acquire) == 0
    }

    pub(crate) fn progress(&self) -> crate::progress::SchedulerProgress {
        #[cfg(feature = "diagnostics")]
        for (domain, metrics) in self.metrics.iter().enumerate() {
            self.trace(
                "scheduler.domain.acquisitions",
                domain as u64,
                metrics.acquisitions.load(Ordering::Relaxed),
            );
            self.trace(
                "scheduler.domain.wait_ns",
                domain as u64,
                metrics.wait_ns.load(Ordering::Relaxed),
            );
            self.trace(
                "scheduler.domain.hold_ns",
                domain as u64,
                metrics.hold_ns.load(Ordering::Relaxed),
            );
        }
        let mut snapshot = crate::progress::SchedulerProgress {
            demand_pending: self.demand_pending.load(Ordering::Acquire)
                || self
                    .demand_updates_pending
                    .iter()
                    .any(|pending| pending.load(Ordering::Acquire)),
            provider_callbacks: self.provider_callbacks.load(Ordering::Acquire),
            records_full: self.accounting.records.load(Ordering::Acquire) >= self.limits.records,
            edges_full: self.limits.edges != 0
                && self.accounting.edges.load(Ordering::Acquire) >= self.limits.edges,
            ..Default::default()
        };
        for class in ExecutionClass::ALL {
            let state = self.classes[class.index()]
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let index = class.index();
            snapshot.handoff_wrappers[index] = state.ready.handed_off;
            snapshot.runnable_full[index] =
                state.ready.runnable + state.attaching_runnable >= self.limits.runnable[index];
            snapshot.handoff_full[index] = state.ready.handed_off >= self.limits.handoff[index];
            for job in state.records.values() {
                if let Some(record) = job.record_lock().as_ref() {
                    match record.stage {
                        Stage::Waiting => snapshot.waiting += 1,
                        Stage::DeferredReady => snapshot.deferred += 1,
                        Stage::Ready => snapshot.ready += 1,
                        Stage::Handed => snapshot.handed += 1,
                        Stage::Running => snapshot.running += 1,
                        Stage::Finalizing => snapshot.finalizing += 1,
                    }
                }
            }
        }
        snapshot
    }

    fn reserve_group_id(&self, class: ExecutionClass) -> Result<u64, SpawnError> {
        // Identity reservation uses the group's own acceptance gate.
        let _gate = self.class_lock(class);
        if self.abandoned.load(Ordering::Acquire) {
            return Err(SpawnError::Closed);
        }
        Ok(Self::allocate_id(&self.next_group))
    }
    pub(crate) fn prepare_stage(
        &self,
        runtime: u64,
        options: &crate::execution::StageOptions<'_>,
    ) -> Result<(SubmitExtras, Option<ByteLease>), SpawnError> {
        if options.work_set.is_some() && options.discovery.is_some() {
            return Err(SpawnError::InvalidContext);
        }
        let work_set = match (options.work_set, options.discovery) {
            (Some(set), _) => Some(set.try_root_lease(runtime)),
            (_, Some(permit)) => Some(permit.try_child_lease(runtime)),
            _ => None,
        }
        .transpose()
        .map_err(|error| match error {
            DiscoveryError::Full => SpawnError::Full,
            _ => SpawnError::Closed,
        })?;
        let cost = Cost::new(
            options.cost.records.max(1),
            options.cost.edges.max(options.prerequisites.len()),
            options
                .cost
                .deliveries
                .max(usize::from(options.delivery.is_some()))
                - usize::from(
                    options
                        .delivery
                        .as_ref()
                        .is_some_and(|ticket| ticket.is_accounted()),
                ),
            options.cost.bytes,
        );
        if options.retained_bytes > cost.bytes {
            return Err(SpawnError::TooLarge);
        }
        let capacity = if let Some(reservation) = options.reservation {
            if reservation.runtime_identity() != runtime {
                return Err(SpawnError::InvalidReservation);
            }
            Some(reservation.stage(cost).map_err(map_reservation_error)?)
        } else if let Some(pool) = &self.capacity {
            Some(
                pool.try_reserve_ordinary(cost)
                    .map_err(map_reservation_error)?,
            )
        } else {
            if cost.bytes != 0 {
                return Err(SpawnError::Disabled);
            }
            None
        };
        let bytes = if options.retained_bytes != 0 {
            Some(
                capacity
                    .as_ref()
                    .expect("retained bytes require capacity")
                    .retain_bytes(options.retained_bytes)
                    .map_err(map_reservation_error)?,
            )
        } else {
            None
        };
        Ok((
            SubmitExtras {
                work_set,
                capacity: capacity.map(OwnedCapacity::Reservation),
                priority: options.priority,
                reserved: options.reservation.is_some(),
                batch_member: false,
            },
            bytes,
        ))
    }

    pub(crate) fn group(self: &Arc<Self>, class: ExecutionClass) -> Result<Group, SpawnError> {
        let control = self.control.upgrade().ok_or(SpawnError::Closed)?;
        let admission = control.admit_owned(false).map_err(|error| match error {
            crate::execution::ExecutionError::Closed => SpawnError::Closed,
            crate::execution::ExecutionError::InvalidContext => SpawnError::InvalidContext,
            crate::execution::ExecutionError::ClassDisabled(class) => {
                SpawnError::ClassDisabled(class)
            }
        })?;
        let id = self.reserve_group_id(class)?;
        let inner = self.groups.acquire(id, class);
        inner.set_notification_runtime(control.identity());
        if let Some(domain) = control.notification_domain() {
            inner.set_notification_source(domain);
        }
        // The reserved ID is the admission point. Keep the runtime admission
        // alive through checkout even when graceful closure races this call.
        drop(admission);
        Ok(Group::new(inner, Arc::downgrade(self), control.identity()))
    }

    pub(crate) fn renew_group(self: &Arc<Self>, group: &mut Group) -> Result<(), SpawnError> {
        let control = self.control.upgrade().ok_or(SpawnError::Closed)?;
        if group.runtime != control.identity()
            || !Weak::ptr_eq(&group.scheduler, &Arc::downgrade(self))
        {
            return Err(SpawnError::InvalidGroup);
        }
        let admission = control.admit_owned(false).map_err(|error| match error {
            crate::execution::ExecutionError::Closed => SpawnError::Closed,
            crate::execution::ExecutionError::InvalidContext => SpawnError::InvalidContext,
            crate::execution::ExecutionError::ClassDisabled(class) => {
                SpawnError::ClassDisabled(class)
            }
        })?;
        let id = self.reserve_group_id(group.class())?;
        if !group.try_reset(id) {
            let inner = self.groups.acquire(id, group.class());
            *group = Group::new(inner, Arc::downgrade(self), control.identity());
        }
        group.inner.set_notification_runtime(control.identity());
        if let Some(domain) = control.notification_domain() {
            group.inner.set_notification_source(domain);
        }
        drop(admission);
        Ok(())
    }

    pub(crate) fn submit_payload<P, T>(
        self: &Arc<Self>,
        request: SubmitRequest<'_>,
        payload: P,
        run: fn(P) -> T,
        application_failed: fn(&T) -> bool,
    ) -> SpawnResult<T, P>
    where
        P: Send + 'static,
        T: Send + 'static,
    {
        self.submit_payload_delivering(request, payload, run, application_failed, &mut None)
    }

    pub(crate) fn submit_payload_delivering<P, T>(
        self: &Arc<Self>,
        request: SubmitRequest<'_>,
        payload: P,
        run: fn(P) -> T,
        application_failed: fn(&T) -> bool,
        delivery: &mut Option<crate::owner::DeliveryTicket>,
    ) -> SpawnResult<T, P>
    where
        P: Send + 'static,
        T: Send + 'static,
    {
        self.submit_payload_accounted(
            request,
            payload,
            run,
            application_failed,
            delivery,
            SubmitExtras::default(),
        )
    }

    pub(crate) fn submit_payload_in_set<P: Send + 'static, T: Send + 'static>(
        self: &Arc<Self>,
        request: SubmitRequest<'_>,
        payload: P,
        run: fn(P) -> T,
        application_failed: fn(&T) -> bool,
        lease: WorkSetLease,
    ) -> SpawnResult<T, P> {
        self.submit_payload_accounted(
            request,
            payload,
            run,
            application_failed,
            &mut None,
            SubmitExtras {
                work_set: Some(lease),
                ..SubmitExtras::default()
            },
        )
    }

    pub(crate) fn admit_external<'a, T: Send + 'static>(
        self: &Arc<Self>,
        control: &Arc<RuntimeControl>,
        mut options: ExternalOptions<'a>,
    ) -> ExternalResult<'a, T> {
        macro_rules! reject {
            ($reason:expr) => {
                return Err(ExternalRejected {
                    reason: $reason,
                    options,
                })
            };
        }
        if options.work_set.is_some() && options.discovery.is_some() {
            reject!(SpawnError::InvalidContext);
        }
        let work_set = match (options.work_set, options.discovery) {
            (Some(set), _) => Some(set.try_root_lease(control.identity())),
            (_, Some(permit)) => Some(permit.try_child_lease(control.identity())),
            _ => None,
        }
        .transpose();
        let work_set = match work_set {
            Ok(lease) => lease,
            Err(DiscoveryError::Full) => reject!(SpawnError::Full),
            Err(_) => reject!(SpawnError::Closed),
        };
        let admission = match control.admit_external(work_set.is_some()) {
            Ok(admission) => admission,
            Err(crate::execution::ExecutionError::Closed) => reject!(SpawnError::Closed),
            Err(crate::execution::ExecutionError::InvalidContext) => {
                reject!(SpawnError::InvalidContext)
            }
            Err(crate::execution::ExecutionError::ClassDisabled(class)) => {
                reject!(SpawnError::ClassDisabled(class))
            }
        };
        let unaccounted_delivery = options
            .delivery
            .as_ref()
            .is_some_and(|ticket| !ticket.is_accounted());
        let cost = Cost::new(
            options.cost.records.max(1),
            options.cost.edges,
            options
                .cost
                .deliveries
                .max(usize::from(options.delivery.is_some()))
                - usize::from(
                    options
                        .delivery
                        .as_ref()
                        .is_some_and(|ticket| ticket.is_accounted()),
                ),
            options.cost.bytes,
        );
        if options.retained_bytes > cost.bytes {
            reject!(SpawnError::TooLarge);
        }
        let capacity = if let Some(reservation) = options.reservation {
            if reservation.runtime_identity() != control.identity() {
                reject!(SpawnError::InvalidReservation);
            }
            match reservation.stage(cost) {
                Ok(charge) => Some(charge),
                Err(error) => reject!(map_reservation_error(error)),
            }
        } else if let Some(pool) = &self.capacity {
            match pool.try_reserve_ordinary(cost) {
                Ok(charge) => Some(charge),
                Err(error) => reject!(map_reservation_error(error)),
            }
        } else {
            if cost.bytes != 0 {
                reject!(SpawnError::Disabled);
            }
            None
        };
        let bytes = if options.retained_bytes != 0 {
            match capacity
                .as_ref()
                .expect("retained bytes require capacity")
                .retain_bytes(options.retained_bytes)
            {
                Ok(bytes) => Some(bytes),
                Err(error) => reject!(map_reservation_error(error)),
            }
        } else {
            None
        };
        if options.priority.is_some_and(|rank| {
            !self
                .demand
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .graph
                .contains(rank)
        }) || (options.provider_demand.is_some() && !self.demand_enabled)
        {
            reject!(SpawnError::InvalidPriority);
        }
        let charge = match self.accounting.acquire(0) {
            Ok(charge) => charge,
            Err(reason) => reject!(reason),
        };
        let id = Self::allocate_id(&self.next_id);
        let (task, sink) = Task::pending_with_signal(self.signals.acquire());
        task.set_producer(control.identity(), id, Arc::downgrade(self));
        if let Some(domain) = control.notification_domain() {
            task.set_notification_source(domain);
        }
        let core = Arc::new(ExternalCore::new(sink, bytes));
        let mut state = self.domain_lock(&self.external, 4);
        if self.abandoned.load(Ordering::Acquire) {
            reject!(SpawnError::Closed);
        }
        let delivery_capacity = if unaccounted_delivery {
            capacity.as_ref().map(|capacity| {
                capacity
                    .stage(Cost::new(0, 0, 1, 0))
                    .expect("external stage reserved promised delivery")
            })
        } else {
            None
        };
        if let Some(ticket) = options.delivery.as_mut() {
            let rejection = if ticket.runtime_identity() != control.identity() {
                Some(SpawnError::InvalidDelivery)
            } else {
                ticket.try_commit().err().map(|error| match error {
                    crate::owner::TicketCommitError::Closed => SpawnError::Closed,
                    crate::owner::TicketCommitError::AlreadyCommitted => {
                        SpawnError::InvalidDelivery
                    }
                })
            };
            if let Some(reason) = rejection {
                drop(state);
                reject!(reason);
            }
        }
        let set_registration = work_set.clone();
        let finalizer_core = Arc::clone(&core);
        // Attachment retains the accepted external record until graph and
        // delivery binding finish. Abandonment stages suppression during this
        // phase, and provider/cancellation controls are exposed afterward.
        state.insert(
            id,
            ExternalRecord {
                settling: false,
                attaching: true,
                deferred_status: None,
                settle: Arc::new(move |status| finalizer_core.publish_status(status)),
                admission,
                work_set,
                capacity,
                charge,
            },
        );
        drop(state);
        if self.demand_enabled {
            let mut demand = self.domain_lock(&self.demand, 3);
            demand
                .graph
                .register(id, options.priority, &[], options.provider_demand.take())
                .expect("external rank validated");
            self.demand_pending.store(true, Ordering::Release);
        }
        let bound_ticket = options.delivery.take();
        if let Some(ticket) = bound_ticket {
            if let Some(charge) = delivery_capacity {
                ticket.attach_capacity(charge);
            }
            ticket.bind(task.completion());
        }
        let deferred_status = {
            let mut state = self.domain_lock(&self.external, 4);
            let record = state
                .get_mut(&id)
                .expect("attachment retains external acceptance");
            record.attaching = false;
            record.deferred_status.take()
        };
        if let Some(status) = deferred_status {
            self.settle_external(id, status);
        }
        let weak = Arc::downgrade(self);
        let producer_control = ProducerControl::new(move || {
            if let Some(scheduler) = weak.upgrade() {
                scheduler.settle_external(id, TaskStatus::Cancelled);
            }
        });
        if let Some(registration) = set_registration {
            registration.register_producer(producer_control.clone());
        }
        let producer = Producer {
            scheduler: Arc::downgrade(self),
            id,
            core,
        };
        self.service_demand_automatic();
        Ok((producer, task, producer_control))
    }

    pub(crate) fn claim_external(&self, id: u64, abandonment: bool) -> bool {
        let mut state = self.domain_lock(&self.external, 4);
        if self.abandoned.load(Ordering::Acquire) && !abandonment {
            return false;
        }
        let Some(record) = state.get_mut(&id) else {
            return false;
        };
        if record.settling {
            return false;
        }
        record.settling = true;
        true
    }

    pub(crate) fn finish_external(&self, id: u64, publish: impl FnOnce()) {
        let _context = context::ControlCallbackGuard::enter();
        let publication = catch_unwind(AssertUnwindSafe(publish));
        let record = self.domain_lock(&self.external, 4).remove(&id);
        self.remove_demand(id);
        if let Some(record) = record {
            let ExternalRecord {
                admission,
                work_set,
                capacity,
                settle,
                charge,
                ..
            } = record;
            drop((settle, capacity, work_set, charge, admission));
        }
        if let Err(payload) = publication {
            crate::cleanup::discard_panic(payload);
        }
        // Completing an external or ordinary CPU graph node can dirty its
        // ancestors even when no further CPU arrivals occur. Producers retain
        // an automatic graph route rather than relying on an application pump.
        self.service_demand_chunk(32);
        if let Some(control) = self.control.upgrade()
            && let Ok(scheduler) = control.owned_scheduler()
        {
            scheduler.dispatch_ready();
        }
        self.wake.notify();
    }

    pub(crate) fn settle_external(&self, id: u64, status: TaskStatus) {
        {
            let mut state = self.domain_lock(&self.external, 4);
            let Some(record) = state.get_mut(&id) else {
                return;
            };
            if record.attaching {
                record.deferred_status = Some(status);
                return;
            }
        }
        if !self.claim_external(id, status == TaskStatus::Abandoned) {
            return;
        }
        let settle = self
            .external
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&id)
            .map(|record| Arc::clone(&record.settle));
        if let Some(settle) = settle {
            self.finish_external(id, move || settle(status));
        }
    }
    pub(crate) fn submit_payload_accounted<P: Send + 'static, T: Send + 'static>(
        self: &Arc<Self>,
        request: SubmitRequest<'_>,
        payload: P,
        run: fn(P) -> T,
        application_failed: fn(&T) -> bool,
        delivery: &mut Option<crate::owner::DeliveryTicket>,
        extras: SubmitExtras,
    ) -> SpawnResult<T, P> {
        let SubmitRequest {
            control,
            class,
            group,
            options,
            prerequisites,
            policy,
            allow_inline,
        } = request;
        let reject = |reason, operation| SpawnRejected {
            reason,
            operation,
            options,
        };
        let admission = match control.admit_owned(extras.work_set.is_some()) {
            Ok(admission) => admission,
            Err(crate::execution::ExecutionError::Closed) => {
                return Err(reject(SpawnError::Closed, payload));
            }
            Err(crate::execution::ExecutionError::InvalidContext) => {
                return Err(reject(SpawnError::InvalidContext, payload));
            }
            Err(crate::execution::ExecutionError::ClassDisabled(class)) => {
                return Err(reject(SpawnError::ClassDisabled(class), payload));
            }
        };
        // Provisional capacity and work-set ownership must refund before the
        // admission token publishes its final runtime progress notification.
        let mut extras = extras;
        if extras.batch_member && group.is_some_and(|group| !group.inner.is_open()) {
            return Err(reject(SpawnError::InvalidGroup, payload));
        }
        if extras.capacity.is_none()
            && let Some(capacity) = &self.capacity
        {
            let charge = if delivery
                .as_ref()
                .is_some_and(|ticket| !ticket.is_accounted())
            {
                capacity
                    .try_reserve_ordinary(Cost::new(1, prerequisites.len(), 1, 0))
                    .map(OwnedCapacity::Reservation)
            } else {
                capacity.try_charge_owned(prerequisites.len())
            };
            match charge {
                Ok(charge) => extras.capacity = Some(charge),
                Err(error) => return Err(reject(map_reservation_error(error), payload)),
            }
        }
        if extras.priority.is_some_and(|priority| {
            !self
                .demand
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .graph
                .contains(priority)
        }) {
            return Err(reject(SpawnError::InvalidPriority, payload));
        }
        let charge = match self.accounting.acquire(prerequisites.len()) {
            Ok(charge) => charge,
            Err(reason) => return Err(reject(reason, payload)),
        };
        let id = Self::allocate_id(&self.next_id);
        let (task, sink) = Task::pending_with_signal(self.signals.acquire());
        task.set_producer(control.identity(), id, Arc::downgrade(self));
        if let Some(domain) = control.notification_domain() {
            task.set_notification_source(domain);
        }
        // Prepare recycler storage without capturing the rejectable payload.
        let mut job = self.jobs.prepare(id, Arc::downgrade(self));
        let subscriptions = self
            .subscriptions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .acquire(prerequisites.len());
        let set_registration = extras.work_set.clone();
        let mut membership = group.map(|group| group.inner.reserve_members(1));
        let mut state = self.class_lock(class);
        if self.abandoned.load(Ordering::Acquire) {
            return Err(reject(SpawnError::Closed, payload));
        }
        let saturated = prerequisites.is_empty()
            && state.ready.runnable + state.attaching_runnable >= self.limits.runnable_for(class);
        // Batch acceptance arbitrates runnable capacity before group membership,
        // and retains that capacity while dependency attachment is in progress.
        if extras.batch_member && saturated {
            return Err(reject(SpawnError::Full, payload));
        }
        let group_inner = if let Some(group) = group {
            if group.runtime != control.identity()
                || group.inner.class != class
                || !Weak::ptr_eq(&group.scheduler, &Arc::downgrade(self))
                || prerequisites
                    .iter()
                    .any(|input| input.same_signal(&group.completion()))
            {
                return Err(reject(SpawnError::InvalidGroup, payload));
            }
            if !group.inner.add() {
                return Err(reject(SpawnError::InvalidGroup, payload));
            }
            Some(Arc::clone(&group.inner))
        } else {
            None
        };
        let inline =
            saturated && allow_inline && options.eligibility == CallerEligibility::CallerEligible;
        let reason = if inline
            && context::current().is_some_and(|current| {
                current.runtime != control.identity() || current.class != class
            }) {
            Some(SpawnError::InvalidContext)
        } else if saturated && !inline && !extras.reserved {
            Some(SpawnError::Full)
        } else {
            None
        };
        if let Some(reason) = reason {
            drop(state);
            if let Some(group) = &group_inner {
                group.finish(None);
            }
            return Err(reject(reason, payload));
        }
        // Acceptance retains the existing class -> group/owner-route/capacity
        // order. Their notification scopes defer callbacks beyond the class
        // guard; rejection unwinds provisional charges after releasing it.
        let delivery_capacity = if delivery
            .as_ref()
            .is_some_and(|ticket| !ticket.is_accounted())
        {
            extras.capacity.as_ref().map(|capacity| {
                capacity
                    .stage(Cost::new(0, 0, 1, 0))
                    .expect("stage reserved promised delivery")
            })
        } else {
            None
        };
        if let Some(ticket) = delivery.as_mut() {
            let rejection = if ticket.runtime_identity() != control.identity() {
                Some(SpawnError::InvalidDelivery)
            } else {
                ticket.try_commit().err().map(|error| match error {
                    crate::owner::TicketCommitError::Closed => SpawnError::Closed,
                    crate::owner::TicketCommitError::AlreadyCommitted => {
                        SpawnError::InvalidDelivery
                    }
                })
            };
            if let Some(reason) = rejection {
                drop(state);
                if let Some(group) = &group_inner {
                    group.finish(None);
                }
                return Err(reject(reason, payload));
            }
        }
        let group_id = group_inner.as_ref().map(|group| group.id);
        let priority = extras.priority;
        job.initialize(
            class,
            group_id,
            make_envelope(payload, run, application_failed, sink),
            Record {
                completion: group_inner.as_ref().map(|_| task.completion()),
                group: group_inner.clone(),
                membership: None,
                pending: prerequisites.len(),
                failed: false,
                policy,
                stage: Stage::Waiting,
                eligibility: options.eligibility,
                subscriptions,
                prerequisite_range: None,
                // Every accepted record is visible to abandonment before dependency
                // and graph attachment. Closure, not a callback, ends this phase.
                attaching: true,
                deferred_finish: None,
                admission,
                work_set: extras.work_set,
                capacity: extras.capacity,
                resource: priority.is_some(),
                runnable_reserved: extras.batch_member && prerequisites.is_empty(),
                selection: None,
                charge,
            },
        );
        if extras.batch_member && prerequisites.is_empty() {
            state.attaching_runnable += 1;
        }
        // This insertion is the acceptance point; all later failures settle the
        // accepted responsibility and never return the consumed payload.
        state.records.insert(id, job.clone());
        drop(state);
        if let Some(ticket) = delivery.take() {
            if let Some(capacity) = delivery_capacity {
                ticket.attach_capacity(capacity);
            }
            ticket.bind(task.completion());
        }
        if let Some(group) = &group_inner {
            let key = membership
                .as_mut()
                .expect("group capacity prepared")
                .register(job.downgrade());
            job.record_lock()
                .as_mut()
                .expect("attachment remains open")
                .membership = Some(key);
            debug_assert_eq!(group.id, job.group_id().expect("group identity"));
        }
        drop(membership);
        if self.demand_enabled {
            let parents = prerequisites
                .iter()
                .filter_map(|completion| completion.producer_identity())
                .filter_map(|(runtime, id)| (runtime == control.identity()).then_some(id))
                .collect::<Vec<_>>();
            let mut demand = self.domain_lock(&self.demand, 3);
            demand
                .graph
                .register(id, priority, &parents, None)
                .expect("rank checked before acceptance");
            let selection = demand.graph.selection(id);
            if priority.is_some() {
                demand.targets.insert(id, class);
            }
            self.demand_pending.store(true, Ordering::Release);
            drop(demand);
            if let (Some(record), Some(selection)) = (job.record_lock().as_mut(), selection)
                && record
                    .selection
                    .is_none_or(|previous| previous.version < selection.version)
            {
                record.selection = Some(selection);
            }
        }
        #[cfg(test)]
        {
            let hook = self
                .attachment_hook
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .clone();
            if let Some(hook) = hook {
                hook(job.clone());
            }
        }
        let weak = Arc::downgrade(self);
        let producer = ProducerControl::new(move || {
            if let Some(scheduler) = weak.upgrade() {
                scheduler.suppress_id(class, id, TaskStatus::Cancelled);
            }
        });
        if let Some(registration) = set_registration {
            registration.register_producer(producer.clone());
        }
        for prerequisite in prerequisites {
            let weak = Arc::downgrade(self);
            let target = job.downgrade();
            let subscription = prerequisite.subscribe_cancelable(Box::new(move |status| {
                if let (Some(scheduler), Some(job)) = (weak.upgrade(), target.upgrade()) {
                    enqueue_activation(Activation::Prerequisite(scheduler, job, status));
                }
            }));
            let mut record = job.record_lock();
            if let Some(record) = record.as_mut() {
                record.subscriptions.push(subscription);
            } else {
                drop(record);
                drop(subscription);
            }
        }
        if extras.batch_member {
            self.publish_portion(class, std::slice::from_ref(&job));
            self.dispatch_class(class);
            return Ok((task, producer));
        }
        let deferred_finish = {
            let _notification = self.wake.notification_scope();
            let mut record = job.record_lock();
            record.as_mut().and_then(|record| {
                record.attaching = false;
                record.deferred_finish.take()
            })
        };
        if let Some(finish) = deferred_finish {
            self.finish_record(&job, finish);
        } else {
            self.activate_ready(&job, inline);
        }
        if inline {
            self.claim_and_run(&job, true);
        } else {
            self.dispatch_class(class);
        }
        Ok((task, producer))
    }
    fn push_ready(&self, state: &mut ClassState, job: &JobHandle, record: &Record) {
        Self::index_group(record);
        self.push_ready_unindexed(state, job, record);
    }

    fn push_ready_unindexed(&self, state: &mut ClassState, job: &JobHandle, record: &Record) {
        state.rebalance = true;
        #[cfg(feature = "diagnostics")]
        self.trace("job.ready", job.id(), job.class().index() as u64);
        if record.resource {
            state.ready.push_resource(
                job.id(),
                record
                    .selection
                    .expect("resource graph attached before activation"),
            );
        } else {
            state.ready.push(job.id());
        }
    }

    fn index_group(record: &Record) {
        if record.eligibility == CallerEligibility::CallerEligible
            && let Some(group) = &record.group
        {
            group.mark_ready(
                record
                    .membership
                    .expect("activation follows membership registration"),
            );
        }
    }

    fn unindex_group(record: &Record) {
        if record.eligibility == CallerEligibility::CallerEligible
            && let (Some(group), Some(key)) = (&record.group, record.membership)
        {
            group.mark_unready(key);
        }
    }

    fn push_deferred(&self, state: &mut ClassState, job: &JobHandle, record: &Record) {
        state.deferred.push(
            job.id(),
            record
                .resource
                .then(|| record.selection.expect("attached resource")),
        );
        state.rebalance |= record.resource
            || state.ready.pending.has_resources()
            || state.deferred.has_resources();
    }

    fn activate_prerequisite(self: &Arc<Self>, job: &JobHandle, status: TaskStatus) {
        {
            let _notification = self.wake.notification_scope();
            let mut record = job.record_lock();
            let Some(record) = record.as_mut() else {
                return;
            };
            if record.stage != Stage::Waiting || record.pending == 0 {
                return;
            }
            record.pending -= 1;
            record.failed |= !status.is_success();
            // Attachment closure owns activation until every subscription is installed.
            if record.attaching {
                return;
            }
        }
        self.activate_ready(job, false);
        self.dispatch_class(job.class());
    }

    fn activate_prerequisite_range(
        self: &Arc<Self>,
        members: prerequisite_range::Members,
        status: TaskStatus,
    ) {
        // Recycler upgrades and weak destruction precede class coordination.
        // A callback snapshot may race detachment, so every member still checks
        // its current stage after its identity-safe upgrade.
        let jobs: Vec<_> = members
            .into_iter()
            .flatten()
            .filter_map(|weak| weak.upgrade())
            .collect();
        let Some(first) = jobs.first() else {
            return;
        };
        #[cfg(feature = "diagnostics")]
        self.trace("range.activation", first.id(), jobs.len() as u64);
        let class = first.class();
        let mut detached = Vec::with_capacity(jobs.len());
        let mut suppressed = Vec::with_capacity(jobs.len());
        let mut ready_group = None;
        let mut ready_keys = [None; bulk::QUANTUM];
        let mut ready_count = 0;
        {
            let mut state = self.class_lock(class);
            for job in &jobs {
                let mut guard = job.record_lock();
                let Some(record) = guard.as_mut() else {
                    continue;
                };
                if record.stage != Stage::Waiting || record.pending == 0 {
                    continue;
                }
                record.pending -= 1;
                record.failed |= !status.is_success();
                detached.extend(record.prerequisite_range.take());
                if record.attaching {
                    continue;
                }
                if self.abandoned.load(Ordering::Acquire) {
                    suppressed.push((job, TaskStatus::Abandoned));
                } else if record.failed && record.policy == DependencyPolicy::SuccessOnly {
                    suppressed.push((job, TaskStatus::PrerequisiteFailed));
                } else if state.ready.runnable + state.attaching_runnable
                    < self.limits.runnable_for(class)
                {
                    record.stage = Stage::Ready;
                    self.push_ready_unindexed(&mut state, job, record);
                    if record.eligibility == CallerEligibility::CallerEligible
                        && record.group.is_some()
                    {
                        ready_keys[ready_count] =
                            Some(record.membership.expect("registered group member"));
                        ready_count += 1;
                    }
                    ready_group = record.group.clone();
                } else {
                    record.stage = Stage::DeferredReady;
                    self.push_deferred(&mut state, job, record);
                }
            }
            if let Some(group) = &ready_group {
                // This descriptor contains one bounded same-group portion.
                // Class authority remains held until all ready keys are visible.
                group.mark_ready_many(&ready_keys[..ready_count]);
            }
        }
        drop(detached);
        if let Some(group) = ready_group {
            group.notify_ready();
        }
        for (job, status) in suppressed {
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| self.suppress(job, status))) {
                crate::cleanup::discard_panic(payload);
            }
        }
        self.dispatch_class(class);
    }

    fn activate_ready(self: &Arc<Self>, job: &JobHandle, inline: bool) {
        let class = job.class();
        let (suppress, group) = {
            let mut state = self.class_lock(class);
            let mut record = job.record_lock();
            let Some(record) = record.as_mut() else {
                return;
            };
            if record.stage != Stage::Waiting || record.pending != 0 || record.attaching {
                return;
            }
            if self.abandoned.load(Ordering::Acquire) {
                (Some(TaskStatus::Abandoned), None)
            } else if record.failed && record.policy == DependencyPolicy::SuccessOnly {
                (Some(TaskStatus::PrerequisiteFailed), None)
            } else if inline
                || state.ready.runnable + state.attaching_runnable < self.limits.runnable_for(class)
            {
                record.stage = Stage::Ready;
                self.push_ready(&mut state, job, record);
                (None, record.group.clone())
            } else {
                record.stage = Stage::DeferredReady;
                self.push_deferred(&mut state, job, record);
                (None, None)
            }
        };
        if let Some(group) = group {
            group.notify_ready();
        }
        if let Some(status) = suppress {
            self.suppress(job, status);
        }
    }

    fn suppress_id(self: &Arc<Self>, class: ExecutionClass, id: u64, status: TaskStatus) {
        let job = self.classes[class.index()]
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .records
            .get(&id)
            .cloned();
        if let Some(job) = job {
            self.suppress(&job, status);
        }
    }

    fn suppress(self: &Arc<Self>, job: &JobHandle, mut status: TaskStatus) {
        let class = job.class();
        let (promoted, range_member) = {
            let mut state = self.class_lock(class);
            if self.abandoned.load(Ordering::Acquire) {
                status = TaskStatus::Abandoned;
            }
            let mut guard = job.record_lock();
            let Some(record) = guard.as_mut() else {
                return;
            };
            let stage = record.stage;
            let decrement = match stage {
                Stage::Waiting | Stage::DeferredReady => false,
                Stage::Ready | Stage::Handed => true,
                Stage::Running | Stage::Finalizing => return,
            };
            record.stage = Stage::Finalizing;
            Self::unindex_group(record);
            let range_member = record.prerequisite_range.take();
            match stage {
                Stage::Ready => Self::remove_ready(&mut state, job.id()),
                Stage::DeferredReady => {
                    state.deferred.remove(job.id());
                    state.rebalance |= record.resource
                        || state.ready.pending.has_resources()
                        || state.deferred.has_resources();
                }
                _ => {}
            }
            drop(guard);
            let promoted = if decrement {
                state.ready.runnable -= 1;
                self.promote_locked(&mut state, class)
            } else {
                Vec::new()
            };
            (promoted, range_member)
        };
        drop(range_member);
        for group in promoted {
            group.notify_ready();
        }
        self.settle(job, Decision::Suppress(status));
    }

    fn claim_and_run(self: &Arc<Self>, job: &JobHandle, caller: bool) -> ClaimOutcome {
        let Some(control) = self.control.upgrade() else {
            return ClaimOutcome::Unavailable;
        };
        let class = job.class();
        // Notification capture cleanup cannot obtain an execution lease. A
        // handoff lease is deliberately weaker and never substitutes for this.
        let Ok(_lease) = control.acquire_owned(class) else {
            return ClaimOutcome::Unavailable;
        };
        let promoted = {
            let mut state = self.class_lock(class);
            if self.abandoned.load(Ordering::Acquire) {
                return ClaimOutcome::Unavailable;
            }
            let mut guard = job.record_lock();
            let Some(record) = guard.as_mut() else {
                return ClaimOutcome::Lost;
            };
            if !matches!(record.stage, Stage::Ready | Stage::Handed)
                || (caller && record.eligibility != CallerEligibility::CallerEligible)
            {
                return ClaimOutcome::Lost;
            }
            let was_ready = record.stage == Stage::Ready;
            // Claim and suppression compete under this actual class gate.
            record.stage = Stage::Running;
            Self::unindex_group(record);
            drop(guard);
            if was_ready {
                Self::remove_ready(&mut state, job.id());
            }
            state.ready.runnable -= 1;
            self.promote_locked(&mut state, class)
        };
        for group in promoted {
            group.notify_ready();
        }
        #[cfg(feature = "diagnostics")]
        self.trace("job.claimed", job.id(), job.group_id().unwrap_or(0));
        let _context = ContextGuard::enter_owned(control.identity(), class, job.group_id());
        self.settle(job, Decision::Run);
        ClaimOutcome::Ran
    }

    fn run_job(self: &Arc<Self>, job: &JobHandle) {
        #[cfg(feature = "diagnostics")]
        self.trace("job.backend_entry", job.id(), 0);
        self.claim_and_run(job, false);
    }

    fn decrement_handoff(&self, class: ExecutionClass) {
        self.class_lock(class).ready.handed_off -= 1;
    }

    fn remove_ready(state: &mut ClassState, id: u64) {
        // Helping/cancellation can remove a member other than the next head.
        // In a mixed queue that can expose a resource ahead of the old window.
        if state.ready.pending.peek() != Some(id)
            && (state.ready.pending.has_resources() || state.deferred.has_resources())
        {
            state.rebalance = true;
        }
        state.ready.remove(id);
    }

    fn promote_locked(
        &self,
        state: &mut ClassState,
        class: ExecutionClass,
    ) -> Vec<Arc<GroupInner>> {
        self.apply_demand_updates(state, class);
        let mut notifications = Vec::new();
        // Ordinary progress consumes the existing frontier directly. Only
        // arrivals, rank changes, or out-of-order mixed-route removals repair
        // the unhanded window; the deferred backlog is never scanned.
        if state.rebalance && !state.deferred.is_empty() {
            let mut pending = Vec::new();
            while let Some(id) = state.ready.pop() {
                let job = state.records.get(&id).expect("ready record exists");
                let mut guard = job.record_lock();
                let record = guard.as_mut().expect("ready record remains live");
                record.stage = Stage::DeferredReady;
                state.ready.runnable -= 1;
                Self::unindex_group(record);
                pending.push((
                    id,
                    record
                        .resource
                        .then(|| record.selection.expect("attached resource")),
                ));
            }
            for (id, selection) in pending.into_iter().rev() {
                state.deferred.push_front(id, selection);
            }
        }
        state.rebalance = false;
        while state.ready.runnable + state.attaching_runnable < self.limits.runnable_for(class) {
            let Some(id) = state.deferred.pop() else {
                break;
            };
            let job = state.records.get(&id).expect("deferred record exists");
            let mut record = job.record_lock();
            let record = record.as_mut().expect("deferred record remains live");
            if record.stage != Stage::DeferredReady {
                continue;
            }
            record.stage = Stage::Ready;
            Self::index_group(record);
            if let Some(group) = &record.group {
                notifications.push(Arc::clone(group));
            }
            // Borrow fields directly; no owning job handle is dropped under gate.
            if record.resource {
                state
                    .ready
                    .push_resource(id, record.selection.expect("attached resource"));
            } else {
                state.ready.push(id);
            }
        }
        notifications
    }

    fn cleanup_context(&self, job: &JobHandle) -> Option<ContextGuard> {
        let control = self.control.upgrade()?;
        let (class, group) = (job.class(), job.group_id());
        match context::current() {
            None => Some(ContextGuard::enter_owned(control.identity(), class, group)),
            Some(current)
                if current.runtime == control.identity()
                    && current.class == class
                    && group.is_some() =>
            {
                Some(ContextGuard::enter_owned(control.identity(), class, group))
            }
            Some(_) => None,
        }
    }

    fn settle(self: &Arc<Self>, job: &JobHandle, decision: Decision) {
        let Some(envelope) = job.take_envelope() else {
            return;
        };
        #[cfg(feature = "diagnostics")]
        let _trace_job = self.trace_job(job);
        let _context = self.cleanup_context(job);
        #[cfg(feature = "diagnostics")]
        self.trace(
            if matches!(&decision, Decision::Run) {
                "job.invoke"
            } else {
                "job.suppress"
            },
            job.id(),
            0,
        );
        let finish = match catch_unwind(AssertUnwindSafe(|| envelope.settle(decision))) {
            Ok(finish) => finish,
            Err(payload) => {
                crate::cleanup::discard_panic(payload);
                Box::new(|| {})
            }
        };
        #[cfg(feature = "diagnostics")]
        self.trace("job.body_returned", job.id(), 0);
        {
            let _notification = self.wake.notification_scope();
            if let Some(record) = job.record_lock().as_mut() {
                record.stage = Stage::Finalizing;
            }
        }
        self.finish_record(job, finish);
    }

    fn finish_record(self: &Arc<Self>, job: &JobHandle, finish: Finish) {
        #[cfg(feature = "diagnostics")]
        let _trace_job = match crate::diagnostics::Trace::current_job() {
            Some(current) if current.runtime == self.trace_runtime && current.id == job.id() => {
                None
            }
            _ => Some(self.trace_job(job)),
        };
        let _context = self.cleanup_context(job);
        let (subscriptions, range_member, finish) = {
            let _notification = self.wake.notification_scope();
            let mut record = job.record_lock();
            let Some(record) = record.as_mut() else {
                return;
            };
            let range_member = record.prerequisite_range.take();
            if record.attaching {
                record.deferred_finish = Some(finish);
                (None, range_member, None)
            } else {
                (
                    Some(std::mem::take(&mut record.subscriptions)),
                    range_member,
                    Some(finish),
                )
            }
        };
        // Cancellation drops its weak slot even while attachment retains the
        // logical record and deferred result publication responsibility.
        drop(range_member);
        let Some(finish) = finish else {
            return;
        };
        let mut subscriptions = subscriptions.expect("closed attachment subscriptions");
        subscriptions.clear();
        self.subscriptions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .release(subscriptions);
        #[cfg(feature = "diagnostics")]
        self.trace("job.result_publish.begin", job.id(), 0);
        if let Err(payload) = catch_unwind(AssertUnwindSafe(finish)) {
            crate::cleanup::discard_panic(payload);
        }
        #[cfg(feature = "diagnostics")]
        self.trace("job.result_publish.end", job.id(), 0);
        enqueue_activation(Activation::Finalize(Arc::clone(self), job.clone()));
    }

    fn finalize_record(self: &Arc<Self>, job: JobHandle) {
        let record = {
            let _notification = self.wake.notification_scope();
            job.record_lock().take()
        };
        let Some(record) = record else {
            return;
        };
        let removed = self.class_lock(job.class()).records.remove(&job.id());
        drop(removed);
        self.remove_demand(job.id());
        if let Some(group) = &record.group {
            group.retire_member(record.membership.expect("group member registered"));
        }
        // Strong settlement remains capacity -> work set -> group -> runtime.
        drop(record.capacity);
        drop(record.work_set);
        drop(record.charge);
        if let Some(group) = record.group {
            let status = record
                .completion
                .expect("group member retains its completion")
                .status()
                .expect("settled member outcome");
            #[cfg(feature = "diagnostics")]
            self.trace("job.group_finish", job.id(), group.id);
            group.finish(Some(status));
        }
        drop(record.admission);
        self.dispatch_class(job.class());
    }

    pub(crate) fn help_group(self: &Arc<Self>, group: &Arc<GroupInner>) -> bool {
        while let Some(job) = group.ready_member() {
            match self.claim_and_run(&job, true) {
                ClaimOutcome::Ran => return true,
                // The class-authoritative transition already unlinked this key.
                ClaimOutcome::Lost => {}
                // Failed leases/abandonment leave a live head; do not spin.
                ClaimOutcome::Unavailable => return false,
            }
        }
        false
    }

    pub(crate) fn abandon(self: &Arc<Self>) {
        // Size the snapshot from live charges, never the configured ceiling. No
        // allocation, backend operation, callback or final handle drop occurs
        // under the rare fixed Low/Mid/High/external cut.
        let mut jobs = Vec::new();
        let mut external_ids = Vec::new();
        let mut required = self.accounting.records.load(Ordering::Acquire);
        loop {
            jobs.reserve(required);
            external_ids.reserve(required);
            let _notification = self.wake.notification_scope();
            let low = self.classes[0]
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let mid = self.classes[1]
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let high = self.classes[2]
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let external = self
                .external
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let owned_count = low.records.len() + mid.records.len() + high.records.len();
            if owned_count > jobs.capacity() || external.len() > external_ids.capacity() {
                required = owned_count.max(external.len());
                // No claim cut has been published. Releasing all guards before
                // retrying permits the next allocation and admission callbacks.
                continue;
            }
            self.abandoned.store(true, Ordering::Release);
            for state in [&*low, &*mid, &*high] {
                jobs.extend(state.records.values().cloned());
            }
            external_ids.extend(external.keys().copied());
            break;
        }
        self.retire_scratch();
        self.wake.notify();
        for job in jobs {
            self.suppress(&job, TaskStatus::Abandoned);
        }
        for id in external_ids {
            self.settle_external(id, TaskStatus::Abandoned);
        }
    }

    fn dispatch_class(self: &Arc<Self>, class: ExecutionClass) {
        self.service_demand_chunk(32);
        self.dispatch_class_ready(class);
    }

    fn dispatch_class_ready(self: &Arc<Self>, class: ExecutionClass) {
        let Some(control) = self.control.upgrade() else {
            return;
        };
        loop {
            let mut jobs: [Option<JobHandle>; bulk::QUANTUM] = std::array::from_fn(|_| None);
            let mut selected = 0;
            let promoted = {
                let mut state = self.class_lock(class);
                if self.abandoned.load(Ordering::Acquire) {
                    break;
                }
                let promoted = self.promote_locked(&mut state, class);
                let available = bulk::QUANTUM.min(
                    self.limits
                        .handoff_for(class)
                        .saturating_sub(state.ready.handed_off),
                );
                for _ in 0..available {
                    let Some(id) = state.ready.pop() else {
                        break;
                    };
                    let job = state
                        .records
                        .get(&id)
                        .expect("ready record remains indexed");
                    {
                        let mut record = job.record_lock();
                        let record = record.as_mut().expect("ready record is live");
                        debug_assert!(record.stage == Stage::Ready);
                        record.stage = Stage::Handed;
                    }
                    #[cfg(feature = "diagnostics")]
                    self.trace("job.handed", id, job.group_id().unwrap_or(0));
                    jobs[selected] = Some(job.clone());
                    selected += 1;
                    state.ready.handed_off += 1;
                }
                promoted
            };
            for group in promoted {
                group.notify_ready();
            }
            if selected == 0 {
                break;
            }
            let mut reserved = ReservedHandoffs {
                scheduler: self,
                class,
                remaining: selected,
            };
            let Ok(lease) = control.acquire_handoff(class) else {
                drop(reserved);
                drop(jobs);
                self.abandon();
                break;
            };
            let rejected = if selected == 1 {
                let job = jobs[0]
                    .take()
                    .expect("selected handoff retains its control");
                let handoff = Handoff {
                    scheduler: Arc::downgrade(self),
                    class,
                    completed: false,
                };
                reserved.remaining -= 1;
                if let Err(wrapper) = lease.pool().try_spawn_owned(move || handoff.run(job)) {
                    drop(wrapper);
                    true
                } else {
                    false
                }
            } else {
                let mut wrappers: [Option<_>; bulk::QUANTUM] = std::array::from_fn(|_| None);
                for (slot, wrapper) in jobs.iter_mut().zip(&mut wrappers).take(selected) {
                    let job = slot.take().expect("selected handoff retains its control");
                    let handoff = Handoff {
                        scheduler: Arc::downgrade(self),
                        class,
                        completed: false,
                    };
                    // Transfer before preparing the erased wrapper: unwinding
                    // drops the closure's credit, while reserved owns the rest.
                    reserved.remaining -= 1;
                    *wrapper = Some(lease.pool().prepare_owned(move || handoff.run(job)));
                }
                match lease.pool().try_spawn_prepared_range(wrappers) {
                    Ok(()) => false,
                    Err(wrappers) => {
                        drop(wrappers);
                        true
                    }
                }
            };
            if !rejected {
                #[cfg(feature = "diagnostics")]
                self.trace("backend.portion", class.index() as u64, selected as u64);
                #[cfg(test)]
                {
                    let hook = self
                        .handoff_hook
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .clone();
                    if let Some(hook) = hook {
                        hook(selected);
                    }
                }
            }
            // Each wrapper owns one physical credit. A refused portion returns
            // all its credits before abandonment suppresses logical records.
            drop(reserved);
            drop(lease);
            drop(jobs);
            if rejected {
                self.abandon();
                break;
            }
        }
    }

    fn dispatch_ready(self: &Arc<Self>) {
        for class in ExecutionClass::ALL {
            self.dispatch_class_ready(class);
        }
    }
    pub(crate) fn attach_demand(
        self: &Arc<Self>,
        id: u64,
        priority: Priority,
    ) -> Result<Demand, DemandError> {
        let lease = {
            let mut demand = self.domain_lock(&self.demand, 3);
            if self.abandoned.load(Ordering::Acquire) {
                return Err(DemandError::Closed);
            }
            let lease = demand.graph.attach(id, priority)?;
            self.demand_pending.store(true, Ordering::Release);
            lease
        };
        let weak = Arc::downgrade(self);
        let demand = Demand::new(
            lease,
            Arc::new(move |id, command| {
                let scheduler = weak.upgrade().ok_or(DemandError::Closed)?;
                {
                    let mut demand = scheduler.domain_lock(&scheduler.demand, 3);
                    demand.graph.change(id, command)?;
                    scheduler.demand_pending.store(true, Ordering::Release);
                }
                scheduler.service_demand_automatic();
                Ok(())
            }),
        );
        self.service_demand_automatic();
        Ok(demand)
    }

    fn remove_demand(&self, id: u64) {
        if !self.demand_enabled {
            return;
        }
        let provider = {
            let _notification = self.wake.notification_scope();
            let transfer = self
                .demand_transfer
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let (provider, class) = {
                let mut demand = self.domain_lock(&self.demand, 3);
                let class = demand.targets.remove(&id);
                let provider = demand.graph.remove(id);
                self.demand_pending
                    .store(demand.graph.pending_updates(), Ordering::Release);
                (provider, class)
            };
            if let Some(class) = class {
                let mut updates = self.demand_updates[class.index()]
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                updates.remove(&id);
                self.demand_updates_pending[class.index()]
                    .store(!updates.is_empty(), Ordering::Release);
            }
            drop(transfer);
            provider
        };
        crate::cleanup::discard_value(provider);
    }

    fn apply_demand_updates(&self, state: &mut ClassState, class: ExecutionClass) {
        if !self.demand_updates_pending[class.index()].load(Ordering::Acquire) {
            return;
        }
        let updates = {
            let mut mailbox = self.demand_updates[class.index()]
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let updates = mailbox.drain().collect::<Vec<_>>();
            // Publish the clear before releasing the mailbox, then release it
            // before any record wait. Graph publication cannot wait on records
            // indirectly through a mailbox retained by this class consumer.
            self.demand_updates_pending[class.index()].store(false, Ordering::Release);
            updates
        };
        for (id, selection) in updates {
            let Some(job) = state.records.get(&id) else {
                continue;
            };
            let mut guard = job.record_lock();
            if let Some(record) = guard.as_mut()
                && record
                    .selection
                    .is_none_or(|previous| previous.version < selection.version)
            {
                record.selection = Some(selection);
                if record.stage == Stage::Ready {
                    state.ready.update_resource(id, selection);
                    state.rebalance = true;
                } else if record.stage == Stage::DeferredReady {
                    state.deferred.update_resource(id, selection);
                    state.rebalance = true;
                }
            }
        }
    }

    fn service_demand_chunk(&self, budget: usize) {
        if !self.demand_pending.load(Ordering::Acquire) {
            return;
        }
        let mut providers = Vec::new();
        {
            let _notification = self.wake.notification_scope();
            let transfer = self
                .demand_transfer
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let changes = {
                let mut demand = self.domain_lock(&self.demand, 3);
                let changes = demand.graph.service(budget);
                let mut updates = Vec::with_capacity(changes.len());
                for change in changes {
                    if let Some(class) = demand.targets.get(&change.id) {
                        updates.push((*class, change.id, change.selection));
                    }
                    if let Some(provider) = change.provider {
                        providers.push(provider);
                    }
                }
                self.provider_callbacks
                    .fetch_add(providers.len(), Ordering::AcqRel);
                updates
            };
            // One coalesced entry per live resource node. Removal serializes
            // with this publication and erases its mailbox entry before credits
            // return. IDs never repeat even when control storage is reused.
            for (class, id, selection) in changes {
                let mut updates = self.demand_updates[class.index()]
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                let entry = updates.entry(id).or_insert(selection);
                if entry.version < selection.version {
                    *entry = selection;
                }
                self.demand_updates_pending[class.index()].store(true, Ordering::Release);
            }
            {
                let demand = self.domain_lock(&self.demand, 3);
                self.demand_pending
                    .store(demand.graph.pending_updates(), Ordering::Release);
            }
            drop(transfer);
        }
        let _callbacks = ProviderCallbacks {
            scheduler: self,
            count: providers.len(),
        };
        let _context = context::ControlCallbackGuard::enter();
        for (provider, snapshot) in providers {
            if let Err(payload) = catch_unwind(AssertUnwindSafe(|| provider(snapshot))) {
                crate::cleanup::discard_panic(payload);
            }
            crate::cleanup::discard_value(provider);
        }
    }

    pub(crate) fn service_demand(self: &Arc<Self>, budget: usize) -> bool {
        self.service_demand_chunk(budget);
        self.dispatch_ready();
        self.demand_pending.load(Ordering::Acquire)
            || self
                .demand_updates_pending
                .iter()
                .any(|pending| pending.load(Ordering::Acquire))
    }

    fn service_demand_automatic(self: &Arc<Self>) {
        self.service_demand_chunk(32);
        self.dispatch_ready();
    }
}
pub(crate) fn map_reservation_error(error: ReservationError) -> SpawnError {
    match error {
        ReservationError::Full | ReservationError::InsufficientCredits => SpawnError::Full,
        ReservationError::TooLarge => SpawnError::TooLarge,
        ReservationError::Closed => SpawnError::Closed,
    }
}

pub(crate) fn call_once<F, T>(operation: F) -> T
where
    F: FnOnce() -> T,
{
    operation()
}

pub(crate) fn never_fail<T>(_: &T) -> bool {
    false
}

pub(crate) fn result_is_err<T, E>(result: &Result<T, E>) -> bool {
    result.is_err()
}

fn make_envelope<P, T>(
    payload: P,
    run: fn(P) -> T,
    application_failed: fn(&T) -> bool,
    sink: CompletionSink<T>,
) -> Envelope
where
    P: Send + 'static,
    T: Send + 'static,
{
    Box::new(move |decision| -> Finish {
        finish_payload(payload, run, application_failed, sink, decision)
    })
}

fn finish_payload<P: Send + 'static, T: Send + 'static>(
    payload: P,
    run: fn(P) -> T,
    application_failed: fn(&T) -> bool,
    sink: CompletionSink<T>,
    decision: Decision,
) -> Finish {
    let (outcome, failed) = match decision {
        Decision::Run => match catch_unwind(AssertUnwindSafe(|| run(payload))) {
            Ok(value) => {
                let failed = application_failed(&value);
                (Outcome::Success(value), failed)
            }
            Err(panic) => {
                crate::cleanup::discard_panic(panic);
                (Outcome::Panicked, false)
            }
        },
        Decision::Suppress(status) => {
            drop(payload);
            (
                match status {
                    TaskStatus::Cancelled => Outcome::Cancelled,
                    TaskStatus::PrerequisiteFailed | TaskStatus::ApplicationFailed => {
                        Outcome::PrerequisiteFailed
                    }
                    TaskStatus::Panicked => Outcome::Panicked,
                    TaskStatus::Abandoned | TaskStatus::Succeeded => Outcome::Abandoned,
                },
                false,
            )
        }
    };
    Box::new(move || sink.finish(outcome, failed))
}
