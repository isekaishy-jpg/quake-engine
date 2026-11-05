//! Runtime-associated lanes and caller participation.

mod batch;
pub(crate) mod context;
mod delivery;
pub(crate) mod group;
mod owned;
mod scope;
mod work_set;
pub use work_set::WorkOptions;
mod stage;
pub use stage::{StageOptions, StageRejected, StageResult};

pub use batch::{Batch, BatchError};
pub use context::ExecutionError;
pub use delivery::{DeliveryOptions, DeliverySpawnRejected, DeliverySpawnResult};
pub use group::Group;
pub use scope::{BatchRejected, BranchOutcome, JoinRejected, Lane, Panic};

pub(crate) use context::ContextGuard;
