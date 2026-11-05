//! Owned production attempts with provider acceptance and final-access settlement.

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::policy::AdmissionPermit;

mod association;
pub use association::{
    ProductionAttempt, ProductionMap, ProductionStart, StartRejected, StartRejectionReason,
};

/// Actual provider acceptance, independent of completion and consumer demand.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionPhase {
    Prepared,
    Accepted,
    Rejected,
    Aborted,
}

/// Observation of one attempt; counts represent `engine_cache` access capabilities only.
/// No snapshot certifies physical provider or device quiescence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionSnapshot {
    pub phase: ProductionPhase,
    pub active_accesses: usize,
    pub completion_recorded: bool,
    pub outcome_claimed: bool,
}

/// Owned outcome retrieval, gated by acceptance and all `engine_cache` final-access releases.
pub enum ProductionClaim<O, E> {
    Pending,
    Ready(Result<O, E>),
    Abandoned,
    Rejected,
    Aborted,
    Taken,
}

struct State<O, E> {
    phase: ProductionPhase,
    active_accesses: usize,
    outcome: Option<Result<O, E>>,
    completion_recorded: bool,
    outcome_claimed: bool,
    permit: Option<AdmissionPermit>,
}

impl<O, E> State<O, E> {
    fn settled_permit(&mut self) -> Option<AdmissionPermit> {
        if self.phase != ProductionPhase::Prepared && self.active_accesses == 0 {
            self.permit.take()
        } else {
            None
        }
    }
}

fn lock<O, E>(state: &Mutex<State<O, E>>) -> MutexGuard<'_, State<O, E>> {
    // Guards protect callback-free bookkeeping only. Poison recovery preserves
    // owned attempts during unrelated unwinding rather than dropping obligations.
    state.lock().unwrap_or_else(|poison| poison.into_inner())
}

/// Shared observation of a prepared production attempt.
/// Cloning does not add producer access, demand or completion authority.
/// Methods synchronously acquire a bookkeeping mutex; no executor is required.
pub struct Production<O, E> {
    state: Arc<Mutex<State<O, E>>>,
}
impl<O, E> Clone for Production<O, E> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}
impl<O, E> Production<O, E> {
    /// Installs observation and callback state before any provider invocation.
    /// The returned submission owns inputs until explicit submission or drop.
    pub fn prepare<I>(inputs: I) -> (Self, Submission<I, O, E>) {
        Self::prepare_owned(inputs, None)
    }
    /// Retains a caller-acquired production allowance through final access.
    /// The permit releases on rejection, abort or accepted settlement only after
    /// all access guards end, independently of observer lifetime and outcome claim.
    /// Provider-slot admission remains a separate caller-owned policy domain.
    pub fn prepare_with_permit<I>(
        inputs: I,
        permit: AdmissionPermit,
    ) -> (Self, Submission<I, O, E>) {
        Self::prepare_owned(inputs, Some(permit))
    }
    fn prepare_owned<I>(inputs: I, permit: Option<AdmissionPermit>) -> (Self, Submission<I, O, E>) {
        let state = Arc::new(Mutex::new(State {
            phase: ProductionPhase::Prepared,
            active_accesses: 0,
            outcome: None,
            completion_recorded: false,
            outcome_claimed: false,
            permit,
        }));
        let production = Self {
            state: Arc::clone(&state),
        };
        let submission = Submission {
            inputs: Some(inputs),
            settlement: SettlementGuard {
                state,
                settled: false,
            },
        };
        (production, submission)
    }
    /// Observes readiness separately from acceptance and final producer access.
    pub fn snapshot(&self) -> ProductionSnapshot {
        let state = lock(&self.state);
        ProductionSnapshot {
            phase: state.phase,
            active_accesses: state.active_accesses,
            completion_recorded: state.completion_recorded,
            outcome_claimed: state.outcome_claimed,
        }
    }
    /// Moves an accepted outcome only after every producer/access capability ends.
    /// Completion alone cannot make an output claimable; an accepted producer
    /// that ends without completing reports abandonment after all children end.
    pub fn claim(&self) -> ProductionClaim<O, E> {
        let mut state = lock(&self.state);
        match state.phase {
            ProductionPhase::Prepared => return ProductionClaim::Pending,
            ProductionPhase::Rejected => return ProductionClaim::Rejected,
            ProductionPhase::Aborted => return ProductionClaim::Aborted,
            ProductionPhase::Accepted => {}
        }
        if state.active_accesses != 0 {
            return ProductionClaim::Pending;
        }
        if state.outcome_claimed {
            return ProductionClaim::Taken;
        }
        state.outcome_claimed = true;
        match state.outcome.take() {
            Some(outcome) => ProductionClaim::Ready(outcome),
            None => ProductionClaim::Abandoned,
        }
    }
}

struct SettlementGuard<O, E> {
    state: Arc<Mutex<State<O, E>>>,
    settled: bool,
}
impl<O, E> Drop for SettlementGuard<O, E> {
    fn drop(&mut self) {
        if !self.settled {
            let (outcome, permit) = {
                let mut state = lock(&self.state);
                state.phase = ProductionPhase::Aborted;
                (state.outcome.take(), state.settled_permit())
            };
            // User outcome cleanup is never called under the bookkeeping guard.
            drop(outcome);
            drop(permit);
        }
    }
}

/// Caller-owned inputs awaiting an explicit provider decision.
/// Drop aborts the unsubmitted attempt without starting or waiting for work.
pub struct Submission<I, O, E> {
    // Field order settles unsubmitted input before its bookkeeping obligation.
    inputs: Option<I>,
    settlement: SettlementGuard<O, E>,
}
impl<I, O, E> Submission<I, O, E> {
    /// Invokes a provider on the caller, outside all bookkeeping guards.
    /// Inline completion is staged until the provider returns `Accepted`.
    /// Rejection must return this attempt's root producer; a foreign producer is
    /// preserved as `InvalidRejection`, and the original attempt is aborted.
    /// Provider unwinding aborts submission without revoking escaped access guards.
    pub fn submit(
        mut self,
        provider: impl FnOnce(Producer<I, O, E>) -> ProviderDecision<I, O, E>,
    ) -> SubmissionOutcome<I, O, E> {
        let state = Arc::clone(&self.settlement.state);
        lock(&state).active_accesses = 1;
        let producer = Producer {
            inputs: self.inputs.take(),
            lease: AccessLease {
                state: Arc::clone(&state),
            },
        };
        match provider(producer) {
            ProviderDecision::Accepted => {
                let permit = {
                    let mut state = lock(&state);
                    state.phase = ProductionPhase::Accepted;
                    state.settled_permit()
                };
                self.settlement.settled = true;
                drop(permit);
                SubmissionOutcome::Accepted
            }
            ProviderDecision::Rejected(mut producer) => {
                if !Arc::ptr_eq(&state, &producer.lease.state) {
                    // SettlementGuard marks the original attempt aborted on return.
                    return SubmissionOutcome::InvalidRejection(producer);
                }
                let staged_outcome = {
                    let mut state = lock(&state);
                    state.phase = ProductionPhase::Rejected;
                    state.outcome.take()
                };
                self.settlement.settled = true;
                let inputs = producer.inputs.take();
                drop(producer);
                SubmissionOutcome::Rejected(RejectedSubmission {
                    inputs,
                    staged_outcome,
                    production: Production { state },
                })
            }
        }
    }
}

/// Provider decision made after invocation, distinct from a produced outcome.
pub enum ProviderDecision<I, O, E> {
    Accepted,
    Rejected(Producer<I, O, E>),
}

/// Submission settlement with owned rejected or foreign capabilities preserved.
pub enum SubmissionOutcome<I, O, E> {
    Accepted,
    Rejected(RejectedSubmission<I, O, E>),
    InvalidRejection(Producer<I, O, E>),
}

/// Rejected unconsumed input and any outcome staged by an inline callback.
/// Consumed inputs remain in independent access guards; rejection does not wait
/// for them or revoke their ownership. Observation reports outstanding guards.
pub struct RejectedSubmission<I, O, E> {
    inputs: Option<I>,
    staged_outcome: Option<Result<O, E>>,
    production: Production<O, E>,
}
impl<I, O, E> RejectedSubmission<I, O, E> {
    pub fn production(&self) -> Production<O, E> {
        self.production.clone()
    }
    /// Returns caller-owned unconsumed input and staged outcome without cleanup.
    pub fn into_parts(self) -> (Option<I>, Option<Result<O, E>>) {
        (self.inputs, self.staged_outcome)
    }
}

/// Failure to extend an attempt's accepted access obligations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionError {
    AccessOverflow,
    Settled,
}

/// Rejected child discovery preserving its original inputs.
pub struct ChildRejected<I> {
    inputs: I,
    reason: ProductionError,
}
impl<I> ChildRejected<I> {
    pub fn reason(&self) -> ProductionError {
        self.reason
    }
    pub fn into_inputs(self) -> I {
        self.inputs
    }
}
impl<I> fmt::Debug for ChildRejected<I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChildRejected")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

/// Why a completion returned its original owned outcome.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionRejectionReason {
    AlreadyCompleted,
    Settled,
}

/// Completion rejection that neither drops nor copies the supplied outcome.
pub struct CompletionRejected<O, E> {
    outcome: Result<O, E>,
    reason: CompletionRejectionReason,
}
impl<O, E> CompletionRejected<O, E> {
    pub fn reason(&self) -> CompletionRejectionReason {
        self.reason
    }
    pub fn into_outcome(self) -> Result<O, E> {
        self.outcome
    }
}
impl<O, E> fmt::Debug for CompletionRejected<O, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CompletionRejected")
            .field("reason", &self.reason)
            .finish_non_exhaustive()
    }
}

struct AccessLease<O, E> {
    state: Arc<Mutex<State<O, E>>>,
}
impl<O, E> AccessLease<O, E> {
    fn extend(&self) -> Result<Self, ProductionError> {
        let mut state = lock(&self.state);
        if matches!(
            state.phase,
            ProductionPhase::Rejected | ProductionPhase::Aborted
        ) {
            return Err(ProductionError::Settled);
        }
        let next = state
            .active_accesses
            .checked_add(1)
            .ok_or(ProductionError::AccessOverflow)?;
        state.active_accesses = next;
        Ok(Self {
            state: Arc::clone(&self.state),
        })
    }
    fn child<I>(&self, inputs: I) -> Result<ProductionAccess<I, O, E>, ChildRejected<I>> {
        match self.extend() {
            Ok(lease) => Ok(ProductionAccess {
                inputs: Some(inputs),
                lease,
            }),
            Err(reason) => Err(ChildRejected { inputs, reason }),
        }
    }
}
impl<O, E> Drop for AccessLease<O, E> {
    fn drop(&mut self) {
        let permit = {
            let mut state = lock(&self.state);
            state.active_accesses -= 1;
            state.settled_permit()
        };
        drop(permit);
    }
}

/// Non-cloneable root completion authority retaining input through actual access.
/// Completing records readiness but does not release input or this capability.
/// Retain it or a child access through actual downstream/provider final use;
/// callback return and `engine_cache` capability release are not external quiescence receipts.
pub struct Producer<I, O, E> {
    // Input destruction (including panic unwinding) precedes lease release.
    inputs: Option<I>,
    lease: AccessLease<O, E>,
}
impl<I, O, E> Producer<I, O, E> {
    pub fn inputs(&self) -> Option<&I> {
        self.inputs.as_ref()
    }
    pub fn inputs_mut(&mut self) -> Option<&mut I> {
        self.inputs.as_mut()
    }
    /// Transfers consumed input to an independently counted final-access owner.
    /// Overflow or settlement preserves the input in this producer.
    pub fn consume_inputs(&mut self) -> Result<Option<ProductionAccess<I, O, E>>, ProductionError> {
        if self.inputs.is_none() {
            return Ok(None);
        }
        let lease = self.lease.extend()?;
        Ok(Some(ProductionAccess {
            inputs: self.inputs.take(),
            lease,
        }))
    }
    /// Owns discovered child input or continuation state before handing it onward.
    /// Child access cannot complete the root attempt. Unit input is a continuation.
    pub fn child<J>(&self, inputs: J) -> Result<ProductionAccess<J, O, E>, ChildRejected<J>> {
        self.lease.child(inputs)
    }
    /// Stages success or accepted failure once, without ending producer access.
    /// Late and duplicate completion return their owned outcome to the caller.
    pub fn complete(&mut self, outcome: Result<O, E>) -> Result<(), CompletionRejected<O, E>> {
        let reason = {
            let mut state = lock(&self.lease.state);
            if matches!(
                state.phase,
                ProductionPhase::Rejected | ProductionPhase::Aborted
            ) {
                Some(CompletionRejectionReason::Settled)
            } else if state.completion_recorded {
                Some(CompletionRejectionReason::AlreadyCompleted)
            } else {
                state.completion_recorded = true;
                state.outcome = Some(outcome);
                return Ok(());
            }
        };
        Err(CompletionRejected {
            outcome,
            reason: reason.expect("rejected branch records reason"),
        })
    }
}

/// Non-cloneable ownership of consumed input, child input or continuation state.
/// This extends final-access settlement but carries no root completion authority.
/// Dropping consumer interest does not drop or cancel these guards. Explicitly
/// retain guards through the actual external last access; no scheduling is inferred.
pub struct ProductionAccess<I, O, E> {
    inputs: Option<I>,
    lease: AccessLease<O, E>,
}
impl<I, O, E> ProductionAccess<I, O, E> {
    pub fn inputs(&self) -> Option<&I> {
        self.inputs.as_ref()
    }
    pub fn inputs_mut(&mut self) -> Option<&mut I> {
        self.inputs.as_mut()
    }
    /// Retains nested discovery or continuation input before handoff.
    pub fn child<J>(&self, inputs: J) -> Result<ProductionAccess<J, O, E>, ChildRejected<J>> {
        self.lease.child(inputs)
    }
}
