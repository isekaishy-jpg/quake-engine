//! Passive host observation and a generation-based wake protocol.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Instant;

use crate::execution::context;
use crate::external::ExternalProgress;
use crate::runtime::RuntimeState;
use crate::scheduler::CapacityUsage;

/// CPU and control work still owned by the scheduler. Counts describe current
/// state; full flags identify saturated admission routes, not queued applicants.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SchedulerProgress {
    /// How many are waiting on what comes first.
    pub waiting: usize,
    /// How many are ready to run.
    pub ready: usize,
    /// How many are held back for now.
    pub deferred: usize,
    /// How many are being passed to a worker.
    pub handed: usize,
    /// How many are running.
    pub running: usize,
    /// How many are finishing.
    pub finalizing: usize,
    /// How many handoffs are in place, by class.
    pub handoff_wrappers: [usize; 3],
    /// Whether a change of demand is waiting to be dealt with.
    pub demand_pending: bool,
    /// How many provider callbacks are running.
    pub provider_callbacks: usize,
    /// Whether there's no room for more records.
    pub records_full: bool,
    /// Whether there's no room for more edges.
    pub edges_full: bool,
    /// Whether there's no room for more ready work, by class.
    pub runnable_full: [bool; 3],
    /// Whether there's no room for more handoffs, by class.
    pub handoff_full: [bool; 3],
}

/// Transferable owner-route bookkeeping. Waiting deliveries still need their
/// producer; ready deliveries need a matching host phase; cleanup is owner-local.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OwnerProgress {
    /// How many routes there are.
    pub routes: usize,
    /// How many deliveries are waiting on work.
    pub waiting: usize,
    /// How many deliveries are ready.
    pub ready: usize,
    /// How many deliveries have been taken.
    pub claimed: usize,
    /// How many are being cleaned up.
    pub cleanup: usize,
}

impl OwnerProgress {
    pub(crate) fn add(&mut self, other: Self) {
        self.routes += other.routes;
        self.waiting += other.waiting;
        self.ready += other.ready;
        self.claimed += other.claimed;
        self.cleanup += other.cleanup;
    }
}

/// Work-set responsibilities that can outlive an empty CPU queue.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WorkSetsProgress {
    /// How many sets there are.
    pub sets: usize,
    /// How many are open.
    pub open: usize,
    /// How much work they're waiting on.
    pub active_work: usize,
    /// How many discovery permits there are.
    pub discovery_permits: usize,
}

/// One passive observation assembled from independently locked components,
/// not an atomic global snapshot or proof of drain. Call `wait_for_change` with
/// an explicit host deadline, then take a fresh snapshot. Service pending demand
/// updates and owner/provider work before sleeping; observation does not do so.
#[derive(Clone, Debug)]
pub struct Progress {
    /// How the runtime stands.
    pub state: RuntimeState,
    /// How the scheduler stands.
    pub scheduler: SchedulerProgress,
    /// How capacity stands, if it's counted.
    pub capacity: Option<CapacityUsage>,
    /// How owners stand.
    pub owner: OwnerProgress,
    /// How work sets stand.
    pub work_sets: WorkSetsProgress,
    /// How external work stands.
    pub external: ExternalProgress,
    /// Runtime leases include admitted CPU work, active scoped/owned invocations
    /// and in-progress owner-root admissions; these overlap scheduler counts.
    pub active_leases: usize,
    /// How many times this has changed.
    pub wake_generation: u64,
    wake: Arc<Wake>,
}

impl Progress {
    // Components are sampled independently by the runtime; keeping construction
    // here prevents callers from forging the private wake association.
    #[expect(
        clippy::too_many_arguments,
        reason = "each is a separate part of one wait"
    )]
    pub(crate) fn new(
        state: RuntimeState,
        scheduler: SchedulerProgress,
        capacity: Option<CapacityUsage>,
        owner: OwnerProgress,
        work_sets: WorkSetsProgress,
        external: ExternalProgress,
        active_leases: usize,
        wake: Arc<Wake>,
        wake_generation: u64,
    ) -> Self {
        Self {
            state,
            scheduler,
            capacity,
            owner,
            work_sets,
            external,
            active_leases,
            wake_generation,
            wake,
        }
    }

    /// Blocks only for a state-change signal or the caller's deadline. It
    /// never runs CPU work, provider hooks, or owner callbacks. Any registered
    /// live owner or CPU participant must use explicit progress instead.
    ///
    /// # Errors
    ///
    /// Returns an error if it can't wait from here.
    pub fn wait_for_change(&self, deadline: Instant) -> Result<ProgressWait, ProgressWaitError> {
        if context::passive_wait_forbidden() {
            return Err(ProgressWaitError::InvalidContext);
        }
        let changed = self.wake.wait_since(self.wake_generation, deadline);
        let generation = self.wake.generation();
        Ok(if changed {
            ProgressWait::Changed(generation)
        } else {
            ProgressWait::TimedOut(generation)
        })
    }
}

/// A wake reports a change, not the new progress snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressWait {
    /// It changed, and this is the new count.
    Changed(u64),
    /// It didn't change in time, and this is the count.
    TimedOut(u64),
}

/// Why progress can't be waited on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgressWaitError {
    /// It can't be waited on from here.
    InvalidContext,
}

impl std::fmt::Display for ProgressWaitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("passive progress wait from a CPU or live-owner context")
    }
}

impl std::error::Error for ProgressWaitError {}

/// After the first host sample, generation advances for each notified
/// transition. The condvar is dormant before that sample. Mutators notify
/// after publishing state, and the host samples generation before reading it.
#[derive(Debug)]
pub(crate) struct Wake {
    generation: AtomicU64,
    enabled: AtomicBool,
    lock: Mutex<()>,
    changed: Condvar,
    notification: OnceLock<crate::notification::NotifySource>,
}

impl Wake {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            generation: AtomicU64::new(0),
            enabled: AtomicBool::new(false),
            lock: Mutex::new(()),
            changed: Condvar::new(),
            notification: OnceLock::new(),
        })
    }

    pub(crate) fn install_notification_source(&self, source: crate::notification::NotifySource) {
        assert!(
            self.notification.set(source).is_ok(),
            "progress source installed once"
        );
    }

    pub(crate) fn notification_scope(&self) -> crate::notification::NotificationScope {
        crate::notification::NotificationScope::enter_if(self.notification.get().is_some())
    }

    pub(crate) fn generation(&self) -> u64 {
        // Sequential consistency orders this enable/sample pair against a
        // notifier's increment/enabled check, including their first race.
        self.enabled.store(true, Ordering::SeqCst);
        self.generation.load(Ordering::SeqCst)
    }

    pub(crate) fn notify(&self) {
        // If this precedes the first enable in SeqCst order, the mutation
        // itself precedes the subsequent snapshot. Otherwise incrementing
        // the generation lets the host detect it before sleeping.
        if self.enabled.load(Ordering::SeqCst) {
            self.generation
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |generation| {
                    generation.checked_add(1)
                })
                .expect("progress wake generation exhausted");
            let _guard = self.lock.lock().unwrap_or_else(|error| error.into_inner());
            self.changed.notify_all();
        }
        // Preserve the fixed CV wake before an arbitrary host adapter. Host
        // interest does not activate that CV, and scopes defer adapters past
        // enclosing bookkeeping locks.
        if let Some(source) = self.notification.get() {
            source.publish_if_watched();
        }
    }

    pub(crate) fn wait_since(&self, observed: u64, deadline: Instant) -> bool {
        let mut guard = self.lock.lock().unwrap_or_else(|error| error.into_inner());
        loop {
            if self.generation.load(Ordering::SeqCst) != observed {
                return true;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let result = self.changed.wait_timeout(guard, remaining);
            guard = result.unwrap_or_else(|error| error.into_inner()).0;
        }
    }
}
