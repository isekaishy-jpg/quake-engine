//! Several callers waiting on one value being made.
#![expect(clippy::unwrap_used, reason = "an example keeps to what it shows")]
#![expect(clippy::print_stdout, reason = "an example reports to its console")]

use engine_cache::{
    Admission, CountLimit, Production, ProductionClaim, ProviderDecision, SubmissionOutcome,
};

fn main() {
    // These are separate family policy measures, not worker limits or bytes.
    let production_allowance = Admission::new(CountLimit::new(1));
    let provider_slots = Admission::new(CountLimit::new(1));
    let permit = production_allowance
        .try_acquire()
        .expect("production allowance");
    let (production, submission) =
        Production::<Vec<u8>, &'static str>::prepare_with_permit(vec![2, 4, 6], permit);
    let consumer_observer = production.clone();
    let mut continuation = None;
    let decision = submission.submit(|mut producer| {
        let Ok(provider_slot) = provider_slots.try_acquire() else {
            return ProviderDecision::Rejected(producer);
        };
        // The provider may finish inline, before its acceptance return.
        let result = producer
            .inputs()
            .unwrap()
            .iter()
            .map(|value| value * 2)
            .collect();
        continuation = producer.consume_inputs().expect("access bookkeeping");
        producer.complete(Ok(result)).expect("first completion");
        drop(provider_slot);
        drop(producer);
        ProviderDecision::Accepted
    });
    assert!(matches!(decision, SubmissionOutcome::Accepted));
    // Provider capacity may reopen while a consumed-input continuation remains.
    let provider_slot = provider_slots
        .try_acquire()
        .expect("provider slot released");
    assert!(production_allowance.try_acquire().is_err());
    assert!(matches!(
        consumer_observer.claim(),
        ProductionClaim::Pending
    ));
    assert_eq!(continuation.as_ref().unwrap().inputs().unwrap(), &[2, 4, 6]);
    drop(continuation);
    let ProductionClaim::Ready(Ok(result)) = production.claim() else {
        panic!("accepted output becomes claimable after final access");
    };
    assert_eq!(result, vec![4, 8, 12]);
    // The production allowance releases independently of observer lifetime.
    let _replacement_permit = production_allowance
        .try_acquire()
        .expect("allowance released");
    drop(provider_slot);
    println!("Inline production settled after consumed-input final access.");
}
