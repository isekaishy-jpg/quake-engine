//! Owner-driven association; this does not confer publication authority.

use std::collections::HashMap;
use std::fmt;

use crate::cache::Identity;
use crate::demand::{Demand, DemandHandle};
use crate::policy::{Admission, AdmissionError};

use super::{Production, ProductionPhase, Submission};

struct Association<O, E> {
    demand: Demand,
    current: Option<Production<O, E>>,
}

/// Explicit consumer interest and production association by cache-scoped identity.
///
/// Mutation requires an owner or caller synchronization. Internal map hashing
/// uses identity tokens, never user key callbacks. Operations may allocate and
/// synchronously lock demand, admission and attempt bookkeeping in sequence.
/// Identity membership and publication validity remain the caller's concern.
/// Removing an association or dropping this map never cancels escaped work.
pub struct ProductionMap<K, O, E> {
    associations: HashMap<Identity<K>, Association<O, E>>,
    admission: Admission,
}

impl<K, O, E> ProductionMap<K, O, E> {
    /// An empty map, admitting productions by `admission`.
    pub fn new(admission: Admission) -> Self {
        Self {
            associations: HashMap::new(),
            admission,
        }
    }

    /// Creates independent interest, without starting production.
    pub fn interest(&mut self, identity: Identity<K>, revision: u64, urgency: u32) -> DemandHandle {
        self.associations
            .entry(identity)
            .or_insert_with(|| Association {
                demand: Demand::new(),
                current: None,
            })
            .demand
            .attach(revision, urgency)
    }

    /// Observes demand without creating another consumer.
    pub fn demand(&self, identity: &Identity<K>) -> Option<Demand> {
        self.associations
            .get(identity)
            .map(|entry| entry.demand.clone())
    }

    /// What's being made for `identity`, if anything.
    pub fn current(&self, identity: &Identity<K>) -> Option<Production<O, E>> {
        self.associations
            .get(identity)
            .and_then(|entry| entry.current.clone())
    }

    /// Joins pending or unclaimed completed work, returning unused input intact.
    /// Otherwise explicitly starts one retry with a fresh allowance and state.
    /// Rejection, abort, claimed outcome and settled abandonment permit retry.
    /// Demand is checked at admission; later detachment does not cancel work.
    /// Claiming an outcome ends coalescing for that attempt. Callers coordinate
    /// subsequent cache lookup/publication themselves.
    /// Closing the admission domain rejects new attempts but still permits joining
    /// existing work; accepted producer capabilities retain child discovery.
    /// A joined observer does not reserve the result: another observer can claim
    /// it concurrently. Serialize claim and publication in the owning coordinator.
    ///
    /// When composing with `Publication`, look up a valid cached result first.
    /// Create and retain a publication ticket only for `Started`, before submitting
    /// its provider. `Joined` uses the existing attempt and must not start a new
    /// publication round. Keep the original ticket with that production's outcome.
    /// This map compares identity only, not dependency/source revisions: changed
    /// inputs require explicit supersession or settlement of the old association.
    ///
    /// # Errors
    ///
    /// Hands the input back if nothing wants it, too many are running, or the map has
    /// closed.
    pub fn begin_shared<I>(
        &mut self,
        identity: &Identity<K>,
        inputs: I,
    ) -> Result<ProductionStart<I, O, E>, StartRejected<I>> {
        let Some(entry) = self.associations.get_mut(identity) else {
            return Err(StartRejected {
                inputs,
                reason: StartRejectionReason::NoDemand,
            });
        };
        if entry.demand.snapshot().consumers() == 0 {
            return Err(StartRejected {
                inputs,
                reason: StartRejectionReason::NoDemand,
            });
        }
        if let Some(production) = &entry.current {
            let state = production.snapshot();
            let join = match state.phase {
                ProductionPhase::Prepared => true,
                ProductionPhase::Accepted => {
                    state.active_accesses != 0
                        || (state.completion_recorded && !state.outcome_claimed)
                }
                ProductionPhase::Rejected | ProductionPhase::Aborted => false,
            };
            if join {
                return Ok(ProductionStart::Joined {
                    production: production.clone(),
                    unused_input: inputs,
                });
            }
        }
        let attempt = prepare(&self.admission, inputs)?;
        entry.current = Some(attempt.production.clone());
        Ok(ProductionStart::Started(attempt))
    }

    /// Starts a separate owned candidate without changing shared association.
    /// The caller selects a domain permitting independent candidates and later
    /// arbitrates publication; this operation does not select a winner.
    ///
    /// # Errors
    ///
    /// Hands the input back if nothing wants it, too many are running, or the map has
    /// closed.
    pub fn begin_candidate<I>(
        &self,
        identity: &Identity<K>,
        inputs: I,
    ) -> Result<ProductionAttempt<I, O, E>, StartRejected<I>> {
        if !self
            .associations
            .get(identity)
            .is_some_and(|entry| entry.demand.snapshot().consumers() != 0)
        {
            return Err(StartRejected {
                inputs,
                reason: StartRejectionReason::NoDemand,
            });
        }
        prepare(&self.admission, inputs)
    }

    /// Removes association only. Existing consumers and access owners stay valid;
    /// later interest for this identity creates a new demand group.
    /// Dropping the final unclaimed outcome can invoke its destructor here.
    pub fn remove(&mut self, identity: &Identity<K>) -> bool {
        self.associations.remove(identity).is_some()
    }
}

fn prepare<I, O, E>(
    admission: &Admission,
    inputs: I,
) -> Result<ProductionAttempt<I, O, E>, StartRejected<I>> {
    let permit = match admission.try_acquire() {
        Ok(permit) => permit,
        Err(error) => {
            return Err(StartRejected {
                inputs,
                reason: match error {
                    AdmissionError::AtCapacity => StartRejectionReason::AtCapacity,
                    AdmissionError::Closed => StartRejectionReason::Closed,
                },
            });
        }
    };
    let (production, submission) = Production::prepare_with_permit(inputs, permit);
    Ok(ProductionAttempt {
        production,
        submission,
    })
}

/// Owned submission paired with its already-installed observer.
pub struct ProductionAttempt<I, O, E> {
    /// The production that was started.
    pub production: Production<O, E>,
    /// What the provider needs to take it on.
    pub submission: Submission<I, O, E>,
}

/// Shared work never consumes the joining caller's unused input.
pub enum ProductionStart<I, O, E> {
    /// A new production was started.
    Started(ProductionAttempt<I, O, E>),
    /// One was already running, so this joined it.
    Joined {
        /// The production that was joined.
        production: Production<O, E>,
        /// The input this didn't need, handed back.
        unused_input: I,
    },
}

/// Why a production wasn't started.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartRejectionReason {
    /// Nothing wants it any more.
    NoDemand,
    /// Too many are running.
    AtCapacity,
    /// The map has been closed.
    Closed,
}

/// Admission failure preserving the input for retry or caller cleanup.
pub struct StartRejected<I> {
    inputs: I,
    reason: StartRejectionReason,
}

impl<I> StartRejected<I> {
    /// Why it wasn't started.
    pub fn reason(&self) -> StartRejectionReason {
        self.reason
    }
    /// The inputs, handed back.
    pub fn into_inputs(self) -> I {
        self.inputs
    }
}

impl<I> fmt::Debug for StartRejected<I> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StartRejected")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}
