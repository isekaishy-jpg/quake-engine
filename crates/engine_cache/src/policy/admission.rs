//! Independent count reservations for production or provider operation domains.

use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use super::CountLimit;

/// A coherent count observation, independent of bytes and executor concurrency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdmissionSnapshot {
    pub active: usize,
    pub limit: usize,
    /// Permanent root closure; active reservations remain independently owned.
    pub closed: bool,
}

/// A caller-selected count domain with synchronous checked admission.
///
/// Use separate domains for production attempts and active provider operations.
/// Clones observe and reserve the same domain. Lowering a limit never revokes
/// accepted permits. Permanent closure rejects future reservations even if the
/// limit is raised. Bookkeeping locks never invoke caller code.
#[derive(Clone)]
pub struct Admission {
    state: Arc<Mutex<AdmissionSnapshot>>,
}

impl Admission {
    pub fn new(limit: CountLimit) -> Self {
        Self {
            state: Arc::new(Mutex::new(AdmissionSnapshot {
                active: 0,
                limit: limit.limit(),
                closed: false,
            })),
        }
    }

    /// Reserves one count or leaves the closed/full domain unchanged.
    pub fn try_acquire(&self) -> Result<AdmissionPermit, AdmissionError> {
        let mut state = lock(&self.state);
        if state.closed {
            return Err(AdmissionError::Closed);
        }
        if state.active >= state.limit {
            return Err(AdmissionError::AtCapacity);
        }
        // active < limit <= usize::MAX proves the increment representable.
        state.active += 1;
        Ok(AdmissionPermit {
            domain: self.clone(),
        })
    }

    /// Changes the count limit without canceling existing work or reopening closure.
    pub fn set_limit(&self, limit: CountLimit) {
        lock(&self.state).limit = limit.limit();
    }

    /// Permanently stops new reservations in this domain and all its clones.
    ///
    /// Existing permits and their accepted discovery/access capabilities remain
    /// valid. This operation does not stop providers, execute callbacks, wait for
    /// final use, or retire allocations. Observation of zero active reservations
    /// names this count domain only, not whole-application shutdown.
    pub fn close(&self) {
        lock(&self.state).closed = true;
    }

    pub fn snapshot(&self) -> AdmissionSnapshot {
        *lock(&self.state)
    }
}

impl fmt::Debug for Admission {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let snapshot = self.snapshot();
        formatter
            .debug_struct("Admission")
            .field("snapshot", &snapshot)
            .finish()
    }
}

/// Admission could not reserve another count in the closed or full domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionError {
    AtCapacity,
    Closed,
}

impl fmt::Display for AdmissionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::AtCapacity => "admission count is at capacity",
            Self::Closed => "admission domain is closed",
        })
    }
}
impl Error for AdmissionError {}

/// One nonclone reservation, released by its final owner.
///
/// Move it with the operation it represents. Dropping a provider-slot permit
/// says nothing about child work, input use, or allocation lifetime. Production
/// preparation can bind a separate permit through submission and final access.
pub struct AdmissionPermit {
    domain: Admission,
}

impl fmt::Debug for AdmissionPermit {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AdmissionPermit")
            .field("domain", &self.domain)
            .finish()
    }
}

impl Drop for AdmissionPermit {
    fn drop(&mut self) {
        lock(&self.domain.state).active -= 1;
    }
}

fn lock(state: &Mutex<AdmissionSnapshot>) -> MutexGuard<'_, AdmissionSnapshot> {
    // Only nonpanicking primitive bookkeeping runs while locked.
    state.lock().unwrap_or_else(|poison| poison.into_inner())
}
