//! Observed system CPU topology, separate from application worker policy.

use std::error::Error;
use std::fmt;

/// An active processor group's logical processors belonging to one physical core.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CoreAffinity {
    /// Windows processor group number.
    pub group: u16,
    /// Observed logical processor mask in that group; no affinity is installed.
    pub mask: u64,
}

/// One observed physical processor core.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuCore {
    pub(crate) affinities: Vec<CoreAffinity>,
    pub(crate) efficiency_class: u8,
}

impl CpuCore {
    /// Native group masks for this core.
    pub fn affinities(&self) -> &[CoreAffinity] {
        &self.affinities
    }

    /// Native relative efficiency class. Higher values denote greater intrinsic
    /// performance and lower efficiency; zero alone does not identify a CPU kind.
    pub fn efficiency_class(&self) -> u8 {
        self.efficiency_class
    }
}

/// A snapshot of active system cores, not a process affinity or scheduling grant.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuTopology {
    pub(crate) cores: Vec<CpuCore>,
    pub(crate) logical_processors: usize,
}

impl CpuTopology {
    /// Queries Windows without inventing counts after failure. The snapshot may
    /// become stale after processor hot-add; it does not select worker counts.
    ///
    /// # Errors
    ///
    /// Returns an error if the system won't say, or what it says can't be read.
    pub fn query() -> Result<Self, TopologyError> {
        crate::backend::windows::topology::query()
    }

    /// Every physical core returned by the system query.
    pub fn cores(&self) -> &[CpuCore] {
        &self.cores
    }

    /// Number of active physical cores in this snapshot.
    pub fn physical_cores(&self) -> usize {
        self.cores.len()
    }

    /// Number of active logical processors across the returned processor groups.
    pub fn logical_processors(&self) -> usize {
        self.logical_processors
    }
}

/// Failure to obtain a validated topology snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TopologyError {
    /// Windows rejected the query; the HRESULT and diagnostic are preserved.
    Native {
        /// Windows error converted to HRESULT by the binding.
        code: i32,
        /// Owned native diagnostic.
        message: String,
    },
    /// The buffer kept growing during bounded retries; retry the query later.
    ChangedDuringQuery,
    /// The native buffer had invalid bounds, identities or counts.
    InvalidData(&'static str),
    /// Reserving snapshot storage failed.
    AllocationFailed,
}

impl fmt::Display for TopologyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Native { code, message } => {
                write!(f, "topology query failed ({code:#x}): {message}")
            }
            Self::ChangedDuringQuery => f.write_str("topology kept changing during query"),
            Self::InvalidData(reason) => write!(f, "invalid topology data: {reason}"),
            Self::AllocationFailed => f.write_str("topology storage reservation failed"),
        }
    }
}

impl Error for TopologyError {}
