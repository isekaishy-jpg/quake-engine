//! CPU execution, scoped parallel work, and owner-thread coordination.
//!
//! Three independently configured worker pools support borrowed joins, indexed
//! chunks, and worker/owner overlap with fallible startup and explicit shutdown.
//! Owned jobs support bounded admission, dependencies and retained groups.
//! Thread-bound owners publish results through explicit host phases.
//! External providers fulfill logical outcomes separately from unsafe adapter
//! acknowledgement of physical storage release.
//! Explicit root closure lets hosts service accepted work before joining;
//! passive progress snapshots and deadline-based wakes report remaining work.
//!
//! The source checkout includes an application usage guide at
//! `pubdocs/README.md` and runnable integration examples under `examples/`.
//!
//! Runtime-discarded panic payloads are destroyed inside containment. If that
//! destruction panics, its new payload is deliberately retained to avoid an
//! unbounded disposal/unwind chain. Caller-owned panic payloads stay caller-owned.

#![deny(unsafe_op_in_unsafe_fn)]

mod backend;
mod cleanup;
#[cfg(feature = "diagnostics")]
pub mod diagnostics;
mod execution;
mod external;
mod notification;
mod owner;
mod platform;
mod progress;
mod runtime;
mod scheduler;
mod task;

pub use execution::{
    Batch, BatchError, BatchRejected, BranchOutcome, DeliveryOptions, DeliverySpawnRejected,
    DeliverySpawnResult, ExecutionError, Group, JoinRejected, Lane, Panic, StageOptions,
    StageRejected, StageResult, WorkOptions,
};
pub use external::{
    ExternalAccess, ExternalAccessOptions, ExternalAccessRejected, ExternalActivationRejected,
    ExternalOptions, ExternalPrepared, ExternalProgress, ExternalRejected, ExternalResult,
    Producer,
};
pub use notification::{
    NotifyBinding, NotifyError, NotifyFault, NotifyLimits, NotifyRejected, NotifyRoute, NotifyStamp,
};
pub use owner::{
    CancelResult, Delivery, DeliveryControl, DeliveryStatus, DeliveryTicket, Owner, OwnerControl,
    OwnerError, OwnerRejected, OwnerSender, Phase, PreparedDelivery, PumpBudget, PumpMode,
    PumpReport, ReadyAccess,
};
pub use platform::WorkerSetupError;
pub use progress::{
    OwnerProgress, Progress, ProgressWait, ProgressWaitError, SchedulerProgress, WorkSetsProgress,
};
pub use runtime::config::{
    ConfigError, ExecutionClass, RuntimeConfig, ThreadPriority, WorkerConfig,
};
pub use runtime::{BuildError, Runtime, RuntimeBuilder, RuntimeState, ShutdownError};
pub use scheduler::{
    BatchSpawnOptions, BatchSpawnRejected, BatchSpawnResult, ByteLease, CallerEligibility,
    CapacityUsage, Cost, Demand, DemandError, DemandSnapshot, DependencyPolicy, DiscoveryError,
    DiscoveryPermit, LimitError, Limits, OwnedConfigError, OwnedLimits, Priority, Reservation,
    ReservationError, SpawnError, SpawnOptions, SpawnRejected, SpawnResult, WorkSet,
    WorkSetProgress,
};
pub use task::{
    Completion, Outcome, OutcomeRef, ProducerControl, Retained, Shared, Task, TaskStatus,
    ThenRejected, ThenResult, WaitError,
};
