//! Resource caching and publication services.
//!
//! Keyed storage provides cache-scoped identity, retained backing, scoped views
//! and bounded idle collection. Caller-driven cleanup, declared allocation
//! accounting and independent family policy measures preserve ownership costs.
//! Operations are synchronous; no executor or application-specific types are needed.
//! Shared production retains demand and inputs across explicit provider decisions.
//! Publication validates current authority and declared dependency revisions.
//! See `pubdocs/README.md` and `examples/` for standalone use.

#![deny(unsafe_op_in_unsafe_fn)]

mod accounting;
mod cache;
mod demand;
mod dependency;
mod lifecycle;
mod ownership;
mod policy;
mod pool;
mod production;
mod progress;
mod publication;
mod range;
mod source;
mod storage;

pub use accounting::{
    AccountingDomain, AccountingError, AccountingSnapshot, AllocationCharge, AllocationClass,
    ChargeTransferFailure,
};
pub use cache::{
    Cache, Identity, InstallError, InstallErrorReason, InstallResult, Lookup, RetentionReport,
    StoredPayload,
};
pub use demand::{ConsumerIdentity, Demand, DemandHandle, DemandSnapshot, UrgencyUpdate};
pub use dependency::{
    Dependencies, Dependency, DependencyError, DependencyScope, DependencySnapshot,
};
pub use ownership::{AcquisitionError, Backing, CleanupContext, View};
pub use policy::{
    Admission, AdmissionError, AdmissionPermit, AdmissionSnapshot, ByteLimit,
    CACHE_REFERENCE_PAYLOAD_RETENTION, CACHE_REFERENCE_PRODUCTION_LIMIT,
    CACHE_REFERENCE_PROVIDER_SLOTS, CACHE_REFERENCE_RANGE_ENTRIES, CountLimit, PolicyCounter,
    PolicyCounterError,
};
pub use pool::{Pool, PoolClosed, PoolLease, PoolRejected, PoolReturn, PoolSnapshot};
pub use production::{
    ChildRejected, CompletionRejected, CompletionRejectionReason, Producer, Production,
    ProductionAccess, ProductionAttempt, ProductionClaim, ProductionError, ProductionMap,
    ProductionPhase, ProductionSnapshot, ProductionStart, ProviderDecision, RejectedSubmission,
    StartRejected, StartRejectionReason, Submission, SubmissionOutcome,
};
pub use progress::CleanupSnapshot;
pub use publication::{
    Authority, Publication, PublicationAttempt, PublicationRejected, PublicationRejectionReason,
};
pub use range::{
    RangeAllocation, RangeError, RangeInsert, RangeRead, RangeSnapshot, RangeStore, SourceSnapshot,
};
pub use source::{
    SourceBufferSnapshot, SourceBuffers, SourceInsert, SourceRejected, SourceRejectionReason,
};
