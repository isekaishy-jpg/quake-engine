//! Reusable ownership of successive retained waves in one lane.

use super::{Group, Lane};
use crate::runtime::config::ExecutionClass;
use crate::scheduler::SpawnError;

#[cfg(test)]
#[path = "../../tests/unit/batch.rs"]
mod tests;

/// Why a reusable batch could not start another wave.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BatchError {
    /// The current wave has not been sealed and settled.
    Busy,
    /// Runtime admission rejected the new wave. `Closed` includes runtime
    /// shutdown; `Disabled` means owned scheduling was not configured.
    Admission(SpawnError),
}

/// A persistent owner of successive retained groups in one lane.
///
/// Construction does not admit work. Each successful [`begin`](Self::begin)
/// starts an open wave. The owner retains only its current wave; cloned group
/// handles and completion tokens continue to observe their original wave.
/// Dropping the owner neither seals nor waits for the current wave.
pub struct Batch {
    lane: Lane,
    current: Option<Group>,
}

impl Batch {
    pub(crate) fn new(lane: Lane) -> Self {
        Self {
            lane,
            current: None,
        }
    }

    /// Returns the execution class selected when the owner was created.
    pub fn class(&self) -> ExecutionClass {
        self.lane.class()
    }

    /// Returns the current wave, if one has been started.
    pub fn current(&self) -> Option<&Group> {
        self.current.as_ref()
    }

    /// Starts a distinct open wave after the previous wave is sealed and
    /// settled. A `Busy` result takes precedence over runtime admission.
    /// Failed admission leaves the previous wave available through `current`.
    /// Uses the same admission rules as [`Lane::group`], including accepted
    /// execution descendants during graceful shutdown. This does not wait for
    /// members or run their work on the caller.
    pub fn begin(&mut self) -> Result<&Group, BatchError> {
        if let Some(group) = self.current.as_mut() {
            if !group.is_complete() {
                return Err(BatchError::Busy);
            }
            self.lane
                .control
                .owned_scheduler_for(self.lane.class())
                .map_err(BatchError::Admission)?
                .renew_group(group)
                .map_err(BatchError::Admission)?;
        } else {
            self.current = Some(self.lane.group().map_err(BatchError::Admission)?);
        }
        Ok(self
            .current
            .as_ref()
            .expect("begin installed a current wave"))
    }
}

impl Lane {
    /// Creates a lazy reusable owner for successive retained groups in this
    /// lane. The first wave is admitted by [`Batch::begin`].
    pub fn batch(&self) -> Batch {
        Batch::new(self.clone())
    }
}
