//! External logical outcomes and separately tracked physical accesses.
//!
//! Provider cancellation, abandonment, and timeout cannot release storage still
//! in use. Expose payloads only when access is valid, and retain cleanup until
//! the provider acknowledges release. I/O and graphics backends remain external.

#[expect(
    unsafe_code,
    reason = "physical adapters are an unsafe contract by design"
)]
mod physical;
pub(crate) mod producer;

use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use crate::runtime::RuntimeControl;
use crate::scheduler::work_set::WorkSetLease;
use crate::scheduler::{Cost, DiscoveryPermit, Reservation, SpawnError, WorkSet};

pub use physical::{ExternalAccess, ExternalActivationRejected, ExternalPrepared};
pub use producer::{ExternalOptions, ExternalRejected, ExternalResult, Producer};

/// Admission for a physical accessor, independent of any logical producer.
/// `cost.bytes` covers the retained resource and follows it on release. Records
/// include at least one physical registry entry; edges/deliveries belong to
/// separate logical/owner admission and must be zero here.
#[derive(Default)]
pub struct ExternalAccessOptions<'a> {
    /// The work set it counts towards, if any.
    pub work_set: Option<&'a WorkSet>,
    /// What lets it join a sealed work set, if anything.
    pub discovery: Option<&'a DiscoveryPermit>,
    /// The capacity set aside for it, if any.
    pub reservation: Option<&'a Reservation>,
    /// What it's expected to cost.
    pub cost: Cost,
}

/// Physical admission failed before foreign access began. All inputs remain
/// owned by the caller; no provider action has been initiated.
pub struct ExternalAccessRejected<'a, T> {
    /// Why it wasn't taken.
    pub reason: SpawnError,
    /// The resource, handed back.
    pub resource: T,
    /// The options, handed back.
    pub options: ExternalAccessOptions<'a>,
}

impl<T> std::fmt::Debug for ExternalAccessRejected<'_, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalAccessRejected")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

/// Disjoint external responsibilities. Logical completion cannot decrement the
/// physical counts. Orphaned accesses retain resources until proven release;
/// a lost ticket has no automatic recovery or leak-free shutdown guarantee.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExternalProgress {
    /// How many outcomes are waiting on a provider.
    pub logical: usize,
    /// How many resources are ready to be handed over.
    pub prepared: usize,
    /// How many resources a provider has.
    pub active: usize,
    /// How many resources were left when what held them went away.
    pub orphaned: usize,
}

#[derive(Clone, Copy)]
enum PhysicalState {
    Prepared,
    Active,
    Orphaned,
}

struct Entry {
    state: PhysicalState,
    required: bool,
}

#[derive(Default)]
struct RegistryState {
    next_id: u64,
    entries: HashMap<u64, Entry>,
    ordinary: usize,
    required: usize,
}

pub(crate) struct PhysicalRegistry {
    ordinary_capacity: usize,
    required_capacity: usize,
    state: Mutex<RegistryState>,
}

impl PhysicalRegistry {
    pub(crate) fn new(capacity: Option<NonZeroUsize>, required_capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            ordinary_capacity: capacity.map_or(0, NonZeroUsize::get),
            required_capacity: if capacity.is_some() {
                required_capacity
            } else {
                0
            },
            state: Mutex::new(RegistryState::default()),
        })
    }

    pub(crate) fn reserve(&self, required: bool) -> Result<u64, SpawnError> {
        if self.ordinary_capacity == 0 {
            return Err(SpawnError::Disabled);
        }
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let (count, capacity) = if required {
            (state.required, self.required_capacity)
        } else {
            (state.ordinary, self.ordinary_capacity)
        };
        if capacity == 0 {
            return Err(SpawnError::TooLarge);
        }
        if count >= capacity {
            return Err(SpawnError::Full);
        }
        let id = state.next_id;
        state.next_id = id.checked_add(1).expect("physical identity exhausted");
        state.entries.insert(
            id,
            Entry {
                state: PhysicalState::Prepared,
                required,
            },
        );
        if required {
            state.required += 1;
        } else {
            state.ordinary += 1;
        }
        Ok(id)
    }

    pub(crate) fn activate(&self, id: u64) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let entry = state
            .entries
            .get_mut(&id)
            .expect("physical reservation exists");
        assert!(
            matches!(entry.state, PhysicalState::Prepared),
            "physical activation is single-use"
        );
        entry.state = PhysicalState::Active;
    }

    fn orphan(&self, id: u64) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let entry = state
            .entries
            .get_mut(&id)
            .expect("physical reservation exists");
        entry.state = PhysicalState::Orphaned;
    }

    fn remove(&self, id: u64) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let entry = state
            .entries
            .remove(&id)
            .expect("physical reservation settles once");
        if entry.required {
            state.required -= 1;
        } else {
            state.ordinary -= 1;
        }
    }

    pub(crate) fn progress(&self) -> ExternalProgress {
        let state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let mut progress = ExternalProgress::default();
        for entry in state.entries.values() {
            match entry.state {
                PhysicalState::Prepared => progress.prepared += 1,
                PhysicalState::Active => progress.active += 1,
                PhysicalState::Orphaned => progress.orphaned += 1,
            }
        }
        progress
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entries
            .is_empty()
    }
}

/// Physical accounting never retains executor ownership. The capsule owns this
/// until payload cleanup or explicit transfer, including through abandonment.
pub(crate) struct PhysicalRetention {
    pub(crate) control: Arc<RuntimeControl>,
    pub(crate) registry: Arc<PhysicalRegistry>,
    pub(crate) id: u64,
    pub(crate) work_set: Option<WorkSetLease>,
    pub(crate) capacity: Option<Reservation>,
}

impl PhysicalRetention {
    pub(crate) fn activate(&self) -> Result<(), SpawnError> {
        self.control
            .activate_physical(&self.registry, self.id, self.work_set.as_ref())
    }
    pub(crate) fn mark_orphaned(&self) {
        self.registry.orphan(self.id);
        self.control.notify_progress();
    }
}

impl Drop for PhysicalRetention {
    fn drop(&mut self) {
        drop(self.capacity.take());
        drop(self.work_set.take());
        self.registry.remove(self.id);
        self.control.notify_progress();
    }
}
